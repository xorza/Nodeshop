struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    @builtin(position) position: vec4<f32>,
};

struct VertexUniformBuffer {
    projection: mat4x4<f32>,
    model: mat4x4<f32>
};

struct FragmentUniformBuffer {
    color: vec4<f32>,
};

@group(0)
@binding(0)
var<uniform> vertex_data: VertexUniformBuffer;


@vertex
fn vs_main(
    @location(0) position: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
) -> VertexOutput {
    var result: VertexOutput;
    result.position = vertex_data.projection * vertex_data.model *  position;
    result.tex_coord = tex_coord;
    return result;
}

@group(0)
@binding(1)
var r_color: texture_2d<u32>;


@group(0)
@binding(2)
var<uniform> fragment_data: FragmentUniformBuffer;

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureLoad(r_color, vec2<i32>(vertex.tex_coord * 256.0), 0);
    let v = f32(tex.x) / 255.0;
    return vec4<f32>(v, v, v, 1.0) * fragment_data.color;
}
