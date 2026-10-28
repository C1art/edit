// Draws a texture into a rectangle of the target. Color conversion comes from the
// formats: sampling an sRGB texture decodes to linear, and writing to an sRGB target
// encodes back, so the same shader serves layer compositing and present.

struct Placement {
    // Left, top, right, bottom in normalized device coordinates.
    rect: vec4<f32>,
    // x: opacity; the rest is reserved.
    params: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> placement: Placement;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[index];
    var out: VertexOutput;
    out.position = vec4<f32>(mix(placement.rect.xy, placement.rect.zw, corner), 0.0, 1.0);
    out.uv = corner;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(source, source_sampler, in.uv);
    return vec4<f32>(color.rgb, color.a * placement.params.x);
}
