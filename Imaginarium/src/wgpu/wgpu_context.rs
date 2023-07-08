use std::cell::RefCell;
use std::ops::RangeBounds;

use bytemuck::Pod;
use pollster::FutureExt;
use wgpu::CommandEncoder;
use wgpu::util::DeviceExt;

use crate::color_format::ColorFormat;
use crate::image::{Image, ImageDesc};
use crate::wgpu::math::Vert2D;

fn aligned_size_of_uniform<U: Sized>() -> u64 {
    let uniform_size = std::mem::size_of::<U>();
    let uniform_align = 256;
    let uniform_padded_size = (uniform_size + uniform_align - 1) / uniform_align * uniform_align;

    uniform_padded_size as u64
}


pub(crate) enum Action<'a> {
    RunShader {
        shader: &'a Shader,
        input_textures: Vec<&'a Texture>,
        output_texture: &'a Texture,
        push_constants: &'a [u8],
    },
    ImgToTex(Vec<(&'a Image, &'a Texture)>),
    TexToImg(Vec<(&'a Texture, RefCell<&'a mut Image>)>)
    // textures: Vec<&'a Texture>,
    // images: Vec<RefCell<&'a mut Image>>,
    ,
}

pub(crate) struct WgpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub limits: wgpu::Limits,
    pub rect_one_vb: VertexBuffer,
    pub default_sampler: wgpu::Sampler,
    encoder: RefCell<Option<CommandEncoder>>,
}

impl WgpuContext {
    pub fn new() -> anyhow::Result<WgpuContext> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            dx12_shader_compiler: wgpu::Dx12Compiler::Dxc { dxil_path: None, dxc_path: None },
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .block_on()
            .expect("Unable to find a suitable GPU adapter.");

        assert!(adapter.features().contains(wgpu::Features::PUSH_CONSTANTS));

        let _limits = adapter.limits();
        let limits = wgpu::Limits {
            max_push_constant_size: 256,
            max_texture_dimension_1d: 16384,
            max_texture_dimension_2d: 16384,
            ..Default::default()
        };

        let device_descriptor = wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::PUSH_CONSTANTS,
            limits: limits.clone(),
        };

        let (device, queue) = adapter
            .request_device(&device_descriptor, None)
            .block_on()
            .expect("Unable to find a suitable GPU device.");

        let rect_one_vb = VertexBuffer::from_slice(&device, &Vert2D::rect_one());

        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        Ok(WgpuContext {
            device,
            queue,
            limits,
            rect_one_vb,
            default_sampler,
            encoder: RefCell::new(None),
        })
    }

    pub fn perform(&self, actions: &[Action]) {
        for action in actions.iter() {
            match action {
                Action::RunShader {
                    shader,
                    input_textures,
                    output_texture,
                    push_constants,
                } => {
                    let mut encoder_temp = self.encoder.borrow_mut();
                    let encoder = encoder_temp
                        .get_or_insert_with(|| self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: None,
                        }));

                    self.run_shader(
                        encoder,
                        shader,
                        input_textures,
                        output_texture,
                        push_constants,
                    );
                }
                Action::ImgToTex(img_tex) => {
                    for (image, texture) in img_tex.iter() {
                        if image.desc != texture.desc {
                            panic!("Image and texture must have the same dimensions");
                        }
                        let desc = &image.desc;

                        self.queue.write_texture(
                            texture.texture.as_image_copy(),
                            &image.bytes,
                            wgpu::ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(desc.stride()),
                                rows_per_image: Some(desc.height()),
                            },
                            texture.extent,
                        );
                    }
                }
                Action::TexToImg(tex_img) => {
                    for (texture, image) in tex_img.iter() {
                        let mut image = image.borrow_mut();

                        if image.desc != texture.desc {
                            panic!("Image and texture must have the same dimensions");
                        }
                        let desc = &image.desc;

                        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                            size: desc.size_in_bytes() as wgpu::BufferAddress,
                            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                            label: Some("Read buffer"),
                        });

                        let mut encoder = self.encoder
                            .borrow_mut()
                            .take()
                            .unwrap_or_else(|| self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: None,
                            }));

                        encoder.copy_texture_to_buffer(
                            wgpu::ImageCopyTexture {
                                texture: &texture.texture,
                                mip_level: 0,
                                origin: wgpu::Origin3d::ZERO,
                                aspect: Default::default(),
                            },
                            wgpu::ImageCopyBuffer {
                                buffer: &buffer,
                                layout: wgpu::ImageDataLayout {
                                    offset: 0,
                                    bytes_per_row: Some(desc.stride()),
                                    rows_per_image: Some(desc.height()),
                                },
                            },
                            texture.extent,
                        );
                        self.queue.submit(Some(encoder.finish()));

                        let slice = buffer.slice(..);
                        slice.map_async(wgpu::MapMode::Read, |result| {
                            result.unwrap();
                        });
                        self.device.poll(wgpu::Maintain::Wait);

                        {
                            let data = slice.get_mapped_range();
                            image.bytes = data.to_vec();
                            drop(data);
                        }

                        buffer.unmap();
                    }
                }
            }
        }
    }

    pub fn sync(&self) {
        if let Some(encoder) = self.encoder.replace(None) {
            self.queue.submit(Some(encoder.finish()));
            self.device.poll(wgpu::Maintain::Wait);
        }
    }

    pub(crate) fn create_shader(
        &self,
        shader: &str,
        input_texture_count: u32,
        push_constant_size: u32,
    ) -> Shader {
        let device = &self.device;

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(shader.into()),
        });

        let mut wgpu_bind_group_layout_entries: Vec<wgpu::BindGroupLayoutEntry> = Vec::new();
        wgpu_bind_group_layout_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
            count: None,
        });
        wgpu_bind_group_layout_entries.extend(
            (0..input_texture_count as usize)
                .map(|index| {
                    wgpu::BindGroupLayoutEntry {
                        binding: index as u32 + 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    }
                })
        );

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &wgpu_bind_group_layout_entries,
                label: None,
            });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::VERTEX,
                    range: 0..push_constant_size,
                }],
                label: None,
            });

        let vertex_layout =
            vec![wgpu::VertexFormat::Float32x2, wgpu::VertexFormat::Float32x2];
        let mut vertex_stride: u64 = 0;
        let mut vertex_attributes: Vec<wgpu::VertexAttribute> = Vec::new();
        for (index, entry) in vertex_layout.iter().enumerate() {
            vertex_attributes.push(wgpu::VertexAttribute {
                offset: vertex_stride,
                format: *entry,
                shader_location: index as u32,
            });
            vertex_stride += entry.size();
        }

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: vertex_stride,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: vertex_attributes.as_slice(),
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            label: None,
        });

        Shader {
            module,
            bind_group_layout,
            pipeline,
            input_texture_count,
            push_constant_size,
            vertex_layout,
        }
    }

    pub(crate) fn create_texture(&self, image_desc: ImageDesc) -> Texture {
        let extent = wgpu::Extent3d {
            width: image_desc.width(),
            height: image_desc.height(),
            depth_or_array_layers: 1,
        };

        let usage =
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC;

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::from(image_desc.color_format()),
            usage,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Texture {
            desc: image_desc,
            texture,
            view,
            extent,
        }
    }

    pub(crate) fn run_shader(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        shader: &Shader,
        input_textures: &[&Texture],
        output_texture: &Texture,
        push_constant: &[u8],
    ) {
        assert_eq!(input_textures.len() as u32, shader.input_texture_count);
        assert_eq!(shader.push_constant_size, push_constant.len() as u32);

        let device = &self.device;

        let mut bind_entries: Vec<wgpu::BindGroupEntry> = Vec::new();
        bind_entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Sampler(&self.default_sampler),
        });
        input_textures.iter()
            .enumerate()
            .for_each(|(index, tex)| {
                bind_entries.push(wgpu::BindGroupEntry {
                    binding: index as u32 + 1,
                    resource: wgpu::BindingResource::TextureView(&tex.view),
                });
            });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &shader.bind_group_layout,
            entries: bind_entries.as_slice(),
            label: None,
        });

        {
            let mut render_pass = encoder.begin_render_pass(
                &wgpu::RenderPassDescriptor {
                    color_attachments: &[
                        Some(wgpu::RenderPassColorAttachment {
                            view: &output_texture.view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                                store: true,
                            },
                        }),
                    ],
                    depth_stencil_attachment: None,
                    label: None,
                });

            render_pass.push_debug_group("Prepare data for draw.");

            let pipeline = shader
                .get_pipeline(&output_texture.desc.color_format());
            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.set_push_constants(
                wgpu::ShaderStages::VERTEX,
                0,
                push_constant,
            );

            render_pass.pop_debug_group();

            render_pass.insert_debug_marker("Draw.");
            render_pass.set_vertex_buffer(0, self.rect_one_vb.slice(..));
            render_pass.draw(0..self.rect_one_vb.vert_count, 0..1);
        }
    }
}

impl Drop for WgpuContext {
    fn drop(&mut self) {
        if self.encoder.borrow().is_some() {
            panic!("WgpuContext dropped before encoder was submitted. Try calling WgpuContext::sync().");
        }
    }
}


pub(crate) struct VertexBuffer {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) vert_count: u32,
    pub(crate) stride: u32,
}

impl VertexBuffer {
    pub fn from_vec<V: Pod>(device: &wgpu::Device, data: Vec<V>) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            contents: bytemuck::cast_slice(&data),
            usage: wgpu::BufferUsages::VERTEX,
            label: None,
        });

        VertexBuffer {
            buffer,
            vert_count: data.len() as u32,
            stride: std::mem::size_of::<V>() as u32,
        }
    }
    pub fn from_slice<V: Pod>(device: &wgpu::Device, data: &[V]) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            contents: bytemuck::cast_slice(data),
            usage: wgpu::BufferUsages::VERTEX,
            label: None,
        });

        VertexBuffer {
            buffer,
            vert_count: data.len() as u32,
            stride: std::mem::size_of::<V>() as u32,
        }
    }
    pub fn slice<S: RangeBounds<u64>>(&self, range: S) -> wgpu::BufferSlice {
        self.buffer.slice(range)
    }
}

pub(crate) struct Shader {
    pub module: wgpu::ShaderModule,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub pipeline: wgpu::RenderPipeline,
    pub input_texture_count: u32,
    pub push_constant_size: u32,
    pub vertex_layout: Vec<wgpu::VertexFormat>,
}

impl Shader {
    pub fn get_pipeline(&self, _color_format: &ColorFormat) -> &wgpu::RenderPipeline {
        &self.pipeline
    }
}

pub(crate) struct Texture {
    pub desc: ImageDesc,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub extent: wgpu::Extent3d,
}

impl Texture {
    pub fn write(&self, queue: &wgpu::Queue, image: &Image) -> anyhow::Result<()> {
        if self.desc != image.desc {
            return Err(anyhow::anyhow!("image info mismatch"));
        }

        queue.write_texture(
            self.texture.as_image_copy(),
            &image.bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(self.desc.stride()),
                rows_per_image: Some(self.desc.height()),
            },
            self.extent,
        );

        Ok(())
    }

    pub fn read(&self, device: &wgpu::Device, queue: &wgpu::Queue, image: &mut Image) -> anyhow::Result<()> {
        if self.desc != image.desc {
            return Err(anyhow::anyhow!("image info mismatch"));
        }

        let mut encoder = device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            size: self.desc.size_in_bytes() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
            label: Some("Read buffer"),
        });

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: Default::default(),
            },
            wgpu::ImageCopyBuffer {
                buffer: &buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.desc.stride()),
                    rows_per_image: Some(self.desc.height()),
                },
            },
            self.extent,
        );

        queue.submit(Some(encoder.finish()));


        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.unwrap();
        });
        device.poll(wgpu::Maintain::Wait);

        {
            let data = slice.get_mapped_range();
            image.bytes = data.to_vec();
            drop(data);
        }

        buffer.unmap();

        Ok(())
    }
}
