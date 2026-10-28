// Draws a texture as a placed, rotated and cropped quad. Color conversion comes from
// the formats: sampling an sRGB texture decodes to linear, and writing to an sRGB
// target encodes back, so the same shader serves layer compositing and present.

struct Placement {
    // The uncropped picture's center, then the vector along its width (all NDC).
    center_axis_x: vec4<f32>,
    // The vector down its height, then opacity.
    axis_y_opacity: vec4<f32>,
    // Fractions removed from the left, top, right and bottom.
    crop: vec4<f32>,
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
    let uv = mix(placement.crop.xy, vec2<f32>(1.0) - placement.crop.zw, corners[index]);
    let center = placement.center_axis_x.xy;
    let axis_x = placement.center_axis_x.zw;
    let axis_y = placement.axis_y_opacity.xy;
    var out: VertexOutput;
    out.position = vec4<f32>(center + (uv.x - 0.5) * axis_x + (uv.y - 0.5) * axis_y, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(source, source_sampler, in.uv);
    return vec4<f32>(color.rgb, color.a * placement.axis_y_opacity.z);
}
