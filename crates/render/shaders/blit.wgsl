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
    // Key color (linear), then tolerance; keying is off when the tolerance is negative.
    key: vec4<f32>,
    // Key softness, sharpen amount, invert amount, tint amount.
    amounts: vec4<f32>,
    // Tint black (linear), then vignette amount.
    tint_black: vec4<f32>,
    // Tint white (linear).
    tint_white: vec4<f32>,
    // Vignette midpoint and feather.
    vignette: vec4<f32>,
};

const MID_GREY: f32 = 0.18;
const LUMA: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);
const GAMMA: f32 = 2.2;
// The largest chroma distance between saturated colors is about this.
const CHROMA_RANGE: f32 = 0.5;

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

fn to_gamma(rgb: vec3<f32>) -> vec3<f32> {
    return pow(max(rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / GAMMA));
}

fn to_linear(rgb: vec3<f32>) -> vec3<f32> {
    return pow(max(rgb, vec3<f32>(0.0)), vec3<f32>(GAMMA));
}

// BT.709 Cb and Cr of gamma-encoded color, so keying matches what the eye separates.
fn chroma(rgb: vec3<f32>) -> vec2<f32> {
    let c = to_gamma(rgb);
    return vec2<f32>(dot(c, vec3<f32>(-0.1146, -0.3854, 0.5)), dot(c, vec3<f32>(0.5, -0.4542, -0.0458)));
}

fn key_alpha(rgb: vec3<f32>) -> f32 {
    let key = placement.key;
    if key.w < 0.0 {
        return 1.0;
    }
    let distance = length(chroma(rgb) - chroma(key.rgb));
    let tolerance = key.w * CHROMA_RANGE;
    let softness = max(placement.amounts.x * CHROMA_RANGE, 0.0001);
    return smoothstep(tolerance, tolerance + softness, distance);
}

// Unsharp masking against the four neighbors. Sampling at level 0 is allowed off
// uniform control flow, which the branch needs.
fn sharpen(rgb: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let amount = placement.amounts.y;
    if amount <= 0.0 {
        return rgb;
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(source));
    let around = textureSampleLevel(source, source_sampler, uv + vec2<f32>(texel.x, 0.0), 0.0).rgb
        + textureSampleLevel(source, source_sampler, uv - vec2<f32>(texel.x, 0.0), 0.0).rgb
        + textureSampleLevel(source, source_sampler, uv + vec2<f32>(0.0, texel.y), 0.0).rgb
        + textureSampleLevel(source, source_sampler, uv - vec2<f32>(0.0, texel.y), 0.0).rgb;
    return max(rgb + amount * (rgb - around * 0.25), vec3<f32>(0.0));
}

fn stylize(rgb_in: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let amounts = placement.amounts;
    let mapped = mix(placement.tint_black.rgb, placement.tint_white.rgb, dot(rgb_in, LUMA));
    var rgb = mix(rgb_in, mapped, amounts.w);
    rgb = mix(rgb, to_linear(vec3<f32>(1.0) - to_gamma(rgb)), amounts.z);
    // 0 at the center, 1 in the corners.
    let distance = length(uv - vec2<f32>(0.5)) * sqrt(2.0);
    let midpoint = placement.vignette.x;
    let edge = smoothstep(midpoint, midpoint + max(placement.vignette.y, 0.01), distance);
    let amount = placement.tint_black.w;
    if amount < 0.0 {
        rgb *= 1.0 + amount * edge;
    } else {
        rgb = mix(rgb, vec3<f32>(1.0), amount * edge);
    }
    return rgb;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(source, source_sampler, in.uv);
    let mask = placement.mask;
    let shown = all(in.canvas >= mask.xy) && all(in.canvas <= vec2<f32>(1.0) - mask.zw);
    let keyed = key_alpha(color.rgb);
    // Desaturating the key's edge suppresses the screen's color spilling onto it.
    let unspilled = mix(vec3<f32>(dot(color.rgb, LUMA)), color.rgb, keyed);
    let sharp = sharpen(unspilled, in.uv);
    var rgb = stylize(correct(sharp), in.uv);
    rgb = mix(rgb, vec3<f32>(1.0), placement.balance.w);
    return vec4<f32>(rgb, color.a * keyed * placement.axis_y_opacity.z * f32(shown));
}
