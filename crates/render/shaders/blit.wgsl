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
    // Exposure in stops, contrast, highlights, shadows.
    tone: vec4<f32>,
    // Temperature, tint, saturation, then how far to wash out to white.
    balance: vec4<f32>,
    // Fractions of the canvas hidden from the left, top, right and bottom.
    mask: vec4<f32>,
};

const MID_GREY: f32 = 0.18;
const LUMA: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Where the fragment lies on the canvas: 0..1, from the top left.
    @location(1) canvas: vec2<f32>,
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
    let ndc = center + (uv.x - 0.5) * axis_x + (uv.y - 0.5) * axis_y;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = uv;
    out.canvas = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    return out;
}

// Works on linear light, so exposure is a plain gain and white balance per-channel gains.
fn correct(rgb_in: vec3<f32>) -> vec3<f32> {
    let tone = placement.tone;
    let balance = placement.balance;
    let white = vec3<f32>(1.0 + 0.3 * balance.x, 1.0 - 0.3 * balance.y, 1.0 - 0.3 * balance.x);
    var rgb = rgb_in * white * exp2(tone.x);
    // Contrast pivots on mid grey in log space, as a camera's tone curve does.
    let pivot = vec3<f32>(MID_GREY);
    rgb = pivot * pow(max(rgb, vec3<f32>(0.0)) / pivot, vec3<f32>(1.0 + tone.y));
    let luma = dot(rgb, LUMA);
    let highlight_weight = smoothstep(MID_GREY, 1.0, luma);
    let shadow_weight = 1.0 - smoothstep(0.0, MID_GREY * 2.0, luma);
    rgb *= exp2(tone.z * highlight_weight + tone.w * shadow_weight);
    return mix(vec3<f32>(dot(rgb, LUMA)), rgb, balance.z);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(source, source_sampler, in.uv);
    let mask = placement.mask;
    let shown = all(in.canvas >= mask.xy) && all(in.canvas <= vec2<f32>(1.0) - mask.zw);
    let rgb = mix(correct(color.rgb), vec3<f32>(1.0), placement.balance.w);
    return vec4<f32>(rgb, color.a * placement.axis_y_opacity.z * f32(shown));
}
