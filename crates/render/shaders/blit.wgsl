// Draws a texture as a placed, rotated and cropped quad. Color conversion comes from
// the formats: sampling an sRGB texture decodes to linear, and writing to an sRGB
// target encodes back, so the same shader serves layer compositing and present.

struct Placement {
    // The uncropped picture's center, then the vector along its width (all NDC).
    center_axis_x: vec4<f32>,
    // The vector down its height, opacity, then 1 when the source is premultiplied.
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
    // Whites, blacks, vibrance.
    finish: vec4<f32>,
    // Shadow, midtone and highlight wheels: push toward blue and red, then level.
    wheels: array<vec4<f32>, 3>,
    // Input and look LUT sizes (0 when absent), the look's intensity, then 1 when
    // curves apply.
    grade: vec4<f32>,
};

const MID_GREY: f32 = 0.18;
const LUMA: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);
const GAMMA: f32 = 2.2;
// How far blacks at their limit lift pure black, in linear light.
const BLACK_LIFT: f32 = 0.03;
// Stops a wheel at its rim pushes its range toward the hue, and its level brightens it.
const WHEEL_STOPS: f32 = 1.0;
const LEVEL_STOPS: f32 = 1.0;
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
// Lookups in gamma-encoded values: curves as a row of outputs per channel, then the
// input and look LUTs as cubes indexed by red, green and blue.
@group(1) @binding(0) var curves: texture_2d<f32>;
@group(1) @binding(1) var input_lut: texture_3d<f32>;
@group(1) @binding(2) var look_lut: texture_3d<f32>;
@group(1) @binding(3) var grade_sampler: sampler;

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
    rgb = finish(rgb);
    return saturate_color(rgb, balance.z, placement.finish.z);
}

// Whites scale the top of the range; blacks move the black point, so raising them
// lifts pure black to grey.
fn finish(rgb_in: vec3<f32>) -> vec3<f32> {
    let ends = placement.finish;
    let luma = dot(rgb_in, LUMA);
    var rgb = rgb_in * exp2(ends.x * smoothstep(0.4, 1.0, luma));
    rgb = max(rgb + ends.y * BLACK_LIFT * (1.0 - smoothstep(0.0, MID_GREY, luma)), vec3<f32>(0.0));
    return wheel(rgb);
}

// Each wheel tints and brightens its tonal range: a gain per channel in stops, along
// the BT.709 directions of Cb and Cr, weighted by where the pixel's luma falls.
fn wheel(rgb_in: vec3<f32>) -> vec3<f32> {
    let luma = dot(rgb_in, LUMA);
    let shadows = 1.0 - smoothstep(0.0, MID_GREY * 2.0, luma);
    let highlights = smoothstep(MID_GREY, 1.0, luma);
    let midtones = max(1.0 - shadows - highlights, 0.0);
    var weights = array<f32, 3>(shadows, midtones, highlights);
    var stops = vec3<f32>(0.0);
    for (var i = 0; i < 3; i++) {
        let w = placement.wheels[i];
        let hue = vec3<f32>(1.5748 * w.y, -0.1873 * w.x - 0.4681 * w.y, 1.8556 * w.x);
        stops += weights[i] * (hue * WHEEL_STOPS + vec3<f32>(w.z * LEVEL_STOPS));
    }
    return rgb_in * exp2(stops);
}

// Vibrance saturates in proportion to how muted a pixel already is.
fn saturate_color(rgb: vec3<f32>, saturation: f32, vibrance: f32) -> vec3<f32> {
    let grey = vec3<f32>(dot(rgb, LUMA));
    let peak = max(max(rgb.r, rgb.g), rgb.b);
    let spread = (peak - min(min(rgb.r, rgb.g), rgb.b)) / max(peak, 1e-6);
    let amount = saturation * max(1.0 + vibrance * (1.0 - spread), 0.0);
    return max(mix(grey, rgb, amount), vec3<f32>(0.0));
}

// The sRGB curve exactly, so a LUT sees the code values the source was decoded from.
fn srgb_encode(rgb: vec3<f32>) -> vec3<f32> {
    let c = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}

fn srgb_decode(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

// Texel centers span the cube, so 0 and 1 land exactly on its first and last entries.
fn lookup(lut: texture_3d<f32>, size: f32, rgb: vec3<f32>) -> vec3<f32> {
    let coord = srgb_encode(rgb) * ((size - 1.0) / size) + vec3<f32>(0.5 / size);
    return srgb_decode(max(textureSampleLevel(lut, grade_sampler, coord, 0.0).rgb, vec3<f32>(0.0)));
}

fn input_grade(rgb: vec3<f32>) -> vec3<f32> {
    if placement.grade.x < 2.0 {
        return rgb;
    }
    return lookup(input_lut, placement.grade.x, rgb);
}

fn creative_grade(rgb_in: vec3<f32>) -> vec3<f32> {
    let grade = placement.grade;
    var rgb = rgb_in;
    if grade.y >= 2.0 {
        rgb = mix(rgb, lookup(look_lut, grade.y, rgb), grade.z);
    }
    if grade.w > 0.5 {
        let size = f32(textureDimensions(curves).x);
        let u = srgb_encode(rgb) * ((size - 1.0) / size) + vec3<f32>(0.5 / size);
        rgb = srgb_decode(vec3<f32>(
            textureSampleLevel(curves, grade_sampler, vec2<f32>(u.r, 0.5), 0.0).r,
            textureSampleLevel(curves, grade_sampler, vec2<f32>(u.g, 0.5), 0.0).g,
            textureSampleLevel(curves, grade_sampler, vec2<f32>(u.b, 0.5), 0.0).b,
        ));
    }
    return rgb;
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
    // Neighbours pass through the input LUT too, so it doesn't read as detail.
    let around = input_grade(textureSampleLevel(source, source_sampler, uv + vec2<f32>(texel.x, 0.0), 0.0).rgb)
        + input_grade(textureSampleLevel(source, source_sampler, uv - vec2<f32>(texel.x, 0.0), 0.0).rgb)
        + input_grade(textureSampleLevel(source, source_sampler, uv + vec2<f32>(0.0, texel.y), 0.0).rgb)
        + input_grade(textureSampleLevel(source, source_sampler, uv - vec2<f32>(0.0, texel.y), 0.0).rgb);
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
    var color = textureSample(source, source_sampler, in.uv);
    if placement.axis_y_opacity.w > 0.5 {
        color = vec4<f32>(color.rgb / max(color.a, 1e-6), color.a);
    }
    color = vec4<f32>(input_grade(color.rgb), color.a);
    let mask = placement.mask;
    let shown = all(in.canvas >= mask.xy) && all(in.canvas <= vec2<f32>(1.0) - mask.zw);
    let keyed = key_alpha(color.rgb);
    // Desaturating the key's edge suppresses the screen's color spilling onto it.
    let unspilled = mix(vec3<f32>(dot(color.rgb, LUMA)), color.rgb, keyed);
    let sharp = sharpen(unspilled, in.uv);
    var rgb = stylize(creative_grade(correct(sharp)), in.uv);
    rgb = mix(rgb, vec3<f32>(1.0), placement.balance.w);
    return vec4<f32>(rgb, color.a * keyed * placement.axis_y_opacity.z * f32(shown));
}
