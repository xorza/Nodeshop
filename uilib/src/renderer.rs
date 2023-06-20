use std::borrow::Cow;
use std::f32::consts;
use std::mem;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, UVec2};
use wgpu::{Adapter, Device, Queue, SurfaceConfiguration};
use wgpu::util::DeviceExt;

use crate::app_base::{InitInfo, RenderInfo};
use crate::view::View;

pub trait Renderer {
    fn background(&self);
}

pub(crate) struct WgpuRenderer {
    vertex_buf: wgpu::Buffer,
    vertex_count: u32,
    bind_group: wgpu::BindGroup,
    uniform_buf: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
}

impl Renderer for WgpuRenderer {
    fn background(&self) {}
}


#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    _pos: [f32; 4],
    _color: [f32; 4],
    _tex_coord: [f32; 2],
}

fn vertex(pos: [f32; 3], tc: [f32; 2], col: [f32; 4]) -> Vertex {
    Vertex {
        _pos: [pos[0], pos[1], pos[2], 1.0],
        _color: [col[0], col[1], col[2], col[3]],
        _tex_coord: [tc[0], tc[1]],
    }
}

fn create_vertices() -> Vec<Vertex> {
    // @formatter:off
    let vertex_data = [
        vertex([ 0.0, 0.0, 0.0], [ 0.0, 0.0],[ 1.0, 1.0, 1.0, 1.0]),
        vertex([ 0.3, 0.0, 0.0], [ 1.0, 0.0],[ 1.0, 1.0, 1.0, 1.0]),
        vertex([ 0.3, 0.3, 0.0], [ 1.0, 1.0],[ 1.0, 1.0, 1.0, 1.0]),

        vertex([ 0.0, 0.0, 0.0], [ 0.0, 0.0],[ 1.0, 1.0, 1.0, 1.0]),
        vertex([ 0.3, 0.3, 0.0], [ 1.0, 1.0],[ 1.0, 1.0, 1.0, 1.0]),
        vertex([ 0.0, 0.3, 0.0], [ 0.0, 1.0],[ 1.0, 1.0, 1.0, 1.0]),
    ];
    // @formatter:off

    vertex_data.to_vec()
}

fn create_texels(size: usize) -> Vec<u8> {
    (0..size * size)
        .map(|id| {
            let cx = 3.0 * (id % size) as f32 / (size - 1) as f32 - 2.0;
            let cy = 2.0 * (id / size) as f32 / (size - 1) as f32 - 1.0;
            let (mut x, mut y, mut count) = (cx, cy, 0);
            while count < 0xFF && x * x + y * y < 4.0 {
                let old_x = x;
                x = x * x - y * y + cx;
                y = 2.0 * old_x * y + cy;
                count += 1;
            }
            count
        })
        .collect()
}


fn create_matrix(size: UVec2) -> Mat4 {
    let projection = Mat4::orthographic_rh(
        0.0,
        size.x as f32 / size.y as f32,
        0.0,
        1.0,
        -1.0,
        1.0,
    );

    // let scale = Mat4::from_scale(glam::Vec3::new(1.0, 1.0, -1.0));
    // let translation = Mat4::from_translation(glam::Vec3::new(0.0, 0.0, 0.0));

    projection
}


impl WgpuRenderer {
    pub fn new(init: InitInfo) -> Self {
        let vertex_size = mem::size_of::<Vertex>();
        let vertex_data = create_vertices();

        let vertex_buf = init.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cube Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertex_data),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let bind_group_layout = init.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = init.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let size = 256u32;
        let texels = create_texels(size as usize);
        let texture_extent = wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        };
        let texture = init.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Mandelbrot Set Texture"),
            size: texture_extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        init.queue.write_texture(
            texture.as_image_copy(),
            &texels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(size),
                rows_per_image: None,
            },
            texture_extent,
        );

        let uniform_buf = init.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = init.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
            ],
            label: None,
        });

        let shader = init.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        let vertex_buffers = [wgpu::VertexBufferLayout {
            array_stride: vertex_size as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 4 * 4,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 4 * 8,
                    shader_location: 2,
                },
            ],
        }];

        let pipeline = init.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &vertex_buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(init.surface_config.view_formats[0].into())],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        WgpuRenderer {
            vertex_buf,
            vertex_count: vertex_data.len() as u32,
            bind_group,
            uniform_buf,
            pipeline,
        }
    }
    pub fn render_view(&self, render: RenderInfo, window_size: UVec2, _view: &dyn View) {
        let view_projection = create_matrix(window_size);
        render.queue.write_buffer(
            &self.uniform_buf,
            0,
            bytemuck::cast_slice(view_projection.as_ref()),
        );

        let mut encoder =
            render.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut render_pass = encoder.begin_render_pass(
                &wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: render.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: None,
                });
            render_pass.push_debug_group("Prepare data for draw.");
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buf.slice(..));
            render_pass.pop_debug_group();
            render_pass.insert_debug_marker("Draw!");
            render_pass.draw(0..self.vertex_count, 0..1);
        }

        render.queue.submit(Some(encoder.finish()));
    }
}