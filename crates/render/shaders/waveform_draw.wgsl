// Paints the binned counts as a glowing trace, full white at the top of the picture.

struct Params {
    // Source width and height, columns in use, levels.
    size_columns_levels: vec4<u32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// How quickly a bin saturates, relative to a column spread evenly over all levels.
const GLOW: f32 = 0.35;
const TRACE: vec3<f32> = vec3<f32>(0.45, 1.0, 0.55);

@group(0) @binding(0) var<storage, read> counts: array<u32>;
@group(0) @binding(1) var<uniform> params: Params;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let uv = corners[index];
    var out: VertexOutput;
    out.position = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let size = params.size_columns_levels.xy;
    let columns = params.size_columns_levels.z;
    let levels = params.size_columns_levels.w;
    let column = min(u32(in.uv.x * f32(columns)), columns - 1u);
    let level = min(u32((1.0 - in.uv.y) * f32(levels)), levels - 1u);
    let count = f32(counts[level * columns + column]);
    let pixels_per_column = f32(size.x * size.y) / f32(columns);
    let even_share = pixels_per_column / f32(levels);
    let intensity = 1.0 - exp(-GLOW * count / even_share);
    return vec4<f32>(TRACE * intensity, 1.0);
}
