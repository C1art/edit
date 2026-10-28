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
    // Two transition wipes (see wipe.wgsl): each pattern's number (negative when unused),
    // the side it enters from, its progress and 1 when reversed; then its border's
    // width as a share of the canvas height and its color, linear.
    wipes: array<vec4<f32>, 4>,
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
    // Whites, blacks, vibrance, then the blend mode's number (see blend.wgsl).
    finish: vec4<f32>,
    // Shadow, midtone and highlight wheels: push toward blue and red, then level.
    wheels: array<vec4<f32>, 3>,
    // Input and look LUT sizes (0 when absent), the look's intensity, then 1 when
    // curves apply.
    grade: vec4<f32>,
    // The secondary's key: hue in turns, hue width, softness, then 0 when off, 1 when
    // on and 2 when showing the key.
    secondary_key: vec4<f32>,
    // Its saturation range, then its lightness range.
    secondary_ranges: vec4<f32>,
    // Its temperature, tint, contrast and saturation.
    secondary_balance: vec4<f32>,
    // Its wheel's push toward blue and red and level, then 1 when the key is inverted.
    secondary_wheel: vec4<f32>,
    // The picture fitted to the canvas in canvas pixels, then the number of masks.
    mask_space: vec4<f32>,
    // Three for each mask: center and size as fractions of the picture; rotation in
    // radians, feather and expansion in pixels, opacity; then its shape (0 ellipse,
    // 1 rectangle, 2 path), 1 when inverted and a path's point count.
    masks: array<vec4<f32>, 12>,
    // Each mask's path outline as fractions of the picture, two points to a vec4.
    outlines: array<vec4<f32>, 128>,
    // The picture style (see style.wgsl): brightness, contrast, gamma, posterize levels.
    style_tone: vec4<f32>,
    // Noise amount and seed, then how much the outlines show and 1 when inverted.
    style_grain: vec4<f32>,
    // The offset as fractions of the picture and its share, then lens curvature.
    style_warp: vec4<f32>,
    // Mosaic blocks across and down, then the twirl's angle in radians and its radius.
    style_shape: vec4<f32>,
    // The directional blur's reach in pixels and its direction in radians.
    style_blur: vec4<f32>,
    // Wave height and width in pixels, their direction in radians, then waves travelled.
    style_wave: vec4<f32>,
    // The drop shadow's offset and softness in pixels, then its opacity.
    style_shadow: vec4<f32>,
    // The shadow's color, linear.
    style_shadow_color: vec4<f32>,
    // Warp Stabilizer's shift, turn and scale of the source (see style.wgsl).
    style_stabilize: vec4<f32>,
    // Spherize's center as fractions of the picture, its radius in pixels, then 1 when on.
    style_sphere: vec4<f32>,
    // Emboss's light direction in radians, relief in pixels, contrast and share.
    style_emboss: vec4<f32>,
    // Turbulent Displace's reach and noise size in pixels, its layers and its evolution
    // in turns; then the noise's center as fractions of the picture, and 1 when on.
    style_turbulence: vec4<f32>,
    style_turbulence_center: vec4<f32>,
    // Mirror's center, angle and 1 when on; the warps' map back to the picture, its
    // first row ending in 1 when on, and the warped picture's bounds (see reshape.wgsl).
    reshape_mirror: vec4<f32>,
    reshape_unpin: array<vec4<f32>, 3>,
    reshape_bounds: vec4<f32>,
    // Color Key's encoded color and tolerance, then Leave Color's; Leave Color's amount,
    // softness and 1 when matching hue, then Color Key's softness (see recolor.wgsl).
    recolor_key: vec4<f32>,
    recolor_leave: vec4<f32>,
    recolor_amounts: vec4<f32>,
    // Gradient Ramp's start and end points, each end's encoded color, then 1 when
    // radial and how much of the ramp covers the picture.
    recolor_ramp_points: vec4<f32>,
    recolor_ramp_start: vec4<f32>,
    recolor_ramp_end: vec4<f32>,
    // Channel Mixer's rows: each channel's shares of red, green and blue, then a constant.
    recolor_mix_red: vec4<f32>,
    recolor_mix_green: vec4<f32>,
    recolor_mix_blue: vec4<f32>,
    // Change to Color's encoded colors, its hue tolerance (negative when off) and what
    // it changes; then its lightness and saturation tolerances, its softness, and 1
    // when Channel Mixer is on.
    recolor_change_from: vec4<f32>,
    recolor_change_to: vec4<f32>,
    recolor_change_detail: vec4<f32>,
};

const OUTLINE_VEC4S: u32 = 32u;

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
    // Waves and a shadow reach past the picture, so the quad grows to hold them.
    let shadow = placement.style_shadow;
    let reach = abs(placement.style_wave.x) + select(
        0.0,
        max(abs(shadow.x), abs(shadow.y)) + shadow.z,
        shadow.w > 0.0,
    );
    let margin = vec2<f32>(reach) / picture_size();
    var low = placement.crop.xy - margin;
    var high = vec2<f32>(1.0) - placement.crop.zw + margin;
    if placement.reshape_unpin[0].w > 0.5 {
        low = min(low, placement.reshape_bounds.xy - margin);
        high = max(high, placement.reshape_bounds.zw + margin);
    }
    let uv = mix(low, high, corners[index]);
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

// Hue in turns, saturation and lightness of gamma-encoded color, as a color picker
// shows them.
fn hsl(rgb: vec3<f32>) -> vec3<f32> {
    let c = clamp(to_gamma(rgb), vec3<f32>(0.0), vec3<f32>(1.0));
    let high = max(max(c.r, c.g), c.b);
    let low = min(min(c.r, c.g), c.b);
    let lightness = (high + low) * 0.5;
    let spread = high - low;
    if spread < 1e-5 {
        return vec3<f32>(0.0, 0.0, lightness);
    }
    let saturation = spread / max(1.0 - abs(2.0 * lightness - 1.0), 1e-5);
    var hue: f32;
    if high == c.r {
        hue = (c.g - c.b) / spread;
    } else if high == c.g {
        hue = (c.b - c.r) / spread + 2.0;
    } else {
        hue = (c.r - c.g) / spread + 4.0;
    }
    return vec3<f32>(fract(hue / 6.0), min(saturation, 1.0), lightness);
}

// 1 inside low..high, easing to 0 over `soft` beyond either end.
fn within(value: f32, low: f32, high: f32, soft: f32) -> f32 {
    return smoothstep(low - soft, low, value) * (1.0 - smoothstep(high, high + soft, value));
}

fn secondary_key(rgb: vec3<f32>) -> f32 {
    let key = placement.secondary_key;
    let ranges = placement.secondary_ranges;
    let soft = max(key.z, 1e-3);
    let color = hsl(rgb);
    let hue_distance = abs(fract(color.x - key.x + 0.5) - 0.5);
    // Grey has no hue, so the saturation range alone decides whether it is keyed.
    let hue = 1.0 - smoothstep(key.y, key.y + soft * 0.5, hue_distance);
    let matte = hue * within(color.y, ranges.x, ranges.y, soft) * within(color.z, ranges.z, ranges.w, soft);
    return mix(matte, 1.0 - matte, placement.secondary_wheel.w);
}

// The keyed colors' own white balance, contrast, wheel and saturation, blended in by
// the key.
fn secondary(rgb_in: vec3<f32>) -> vec3<f32> {
    let mode = placement.secondary_key.w;
    if mode < 0.5 {
        return rgb_in;
    }
    let keyed = secondary_key(rgb_in);
    if mode > 1.5 {
        return mix(vec3<f32>(MID_GREY), rgb_in, keyed);
    }
    let balance = placement.secondary_balance;
    let w = placement.secondary_wheel;
    let white = vec3<f32>(1.0 + 0.3 * balance.x, 1.0 - 0.3 * balance.y, 1.0 - 0.3 * balance.x);
    let hue = vec3<f32>(1.5748 * w.y, -0.1873 * w.x - 0.4681 * w.y, 1.8556 * w.x);
    var rgb = rgb_in * white * exp2(hue * WHEEL_STOPS + vec3<f32>(w.z * LEVEL_STOPS));
    let pivot = vec3<f32>(MID_GREY);
    rgb = pivot * pow(max(rgb, vec3<f32>(0.0)) / pivot, vec3<f32>(1.0 + balance.z));
    rgb = saturate_color(rgb, balance.w, 0.0);
    return mix(rgb_in, rgb, keyed);
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
        return color_key_alpha(rgb);
    }
    let distance = length(chroma(rgb) - chroma(key.rgb));
    let tolerance = key.w * CHROMA_RANGE;
    let softness = max(placement.amounts.x * CHROMA_RANGE, 0.0001);
    return smoothstep(tolerance, tolerance + softness, distance) * color_key_alpha(rgb);
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

// Signed distance outside an ellipse, close enough near its outline for a feather.
fn ellipse_distance(p: vec2<f32>, radii: vec2<f32>) -> f32 {
    let scaled = length(p / radii);
    let gradient = length(p / (radii * radii));
    if gradient < 1e-6 {
        return -min(radii.x, radii.y);
    }
    return scaled * (scaled - 1.0) / gradient;
}

fn box_distance(p: vec2<f32>, half_size: vec2<f32>) -> f32 {
    let q = abs(p) - half_size;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
}

fn outline_point(mask: u32, index: u32) -> vec2<f32> {
    let pair = placement.outlines[mask * OUTLINE_VEC4S + index / 2u];
    return select(pair.xy, pair.zw, index % 2u == 1u);
}

// Signed distance outside a mask's path outline in pixels, its sign from the crossings
// of a ray along +x.
fn path_distance(uv: vec2<f32>, mask: u32, count: u32, picture: vec2<f32>) -> f32 {
    let p = uv * picture;
    var nearest = 1e20;
    var side = 1.0;
    var previous = outline_point(mask, count - 1u) * picture;
    for (var i = 0u; i < count; i++) {
        let vertex = outline_point(mask, i) * picture;
        let edge = previous - vertex;
        let to_p = p - vertex;
        let along = clamp(dot(to_p, edge) / max(dot(edge, edge), 1e-6), 0.0, 1.0);
        let gap = to_p - edge * along;
        nearest = min(nearest, dot(gap, gap));
        let crossing = vec3<bool>(
            (p.y >= vertex.y),
            (p.y < previous.y),
            (edge.x * to_p.y > edge.y * to_p.x),
        );
        if all(crossing) || !any(crossing) {
            side = -side;
        }
        previous = vertex;
    }
    return side * sqrt(nearest);
}

// How much of the picture at `uv` the masks keep. Masks add: each covers what the
// others leave by its own opacity.
fn mask_coverage(uv: vec2<f32>) -> f32 {
    let count = u32(placement.mask_space.z);
    if count == 0u {
        return 1.0;
    }
    let picture = placement.mask_space.xy;
    var uncovered = 1.0;
    for (var i = 0u; i < count; i++) {
        let shape = placement.masks[i * 3u];
        let edge = placement.masks[i * 3u + 1u];
        let kind = placement.masks[i * 3u + 2u];
        let offset = (uv - shape.xy) * picture;
        // Turning back by the mask's clockwise rotation, with +y down.
        let sin_cos = vec2<f32>(sin(edge.x), cos(edge.x));
        let p = vec2<f32>(
            sin_cos.y * offset.x + sin_cos.x * offset.y,
            -sin_cos.x * offset.x + sin_cos.y * offset.y,
        );
        let half_size = max(shape.zw * picture * 0.5, vec2<f32>(1e-3));
        var distance = ellipse_distance(p, half_size);
        if kind.x > 1.5 {
            distance = path_distance(uv, i, u32(kind.z), picture);
        } else if kind.x > 0.5 {
            distance = box_distance(p, half_size);
        }
        // A pixel's softness at least, so a hard edge doesn't alias.
        let feather = max(edge.y, 1.0);
        var inside = 1.0 - smoothstep(-0.5 * feather, 0.5 * feather, distance - edge.z);
        if kind.y > 0.5 {
            inside = 1.0 - inside;
        }
        uncovered *= 1.0 - inside * edge.w;
    }
    return 1.0 - uncovered;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return layer_color(in);
}

fn in_crop(uv: vec2<f32>) -> bool {
    return all(uv >= placement.crop.xy) && all(uv <= vec2<f32>(1.0) - placement.crop.zw);
}

// The picture at the warped `uv`, unpremultiplied and through the input LUT.
fn source_color(uv: vec2<f32>) -> vec4<f32> {
    var color = picture_at(uv);
    if placement.axis_y_opacity.w > 0.5 {
        color = vec4<f32>(color.rgb / max(color.a, 1e-6), color.a);
    }
    return ramped(vec4<f32>(input_grade(color.rgb), color.a), uv);
}

// How much of the cropped, keyed picture covers `uv`, which casts the shadow.
fn cast_alpha(uv: vec2<f32>) -> f32 {
    let color = source_color(warp(uv));
    return color.a * key_alpha(color.rgb) * f32(in_crop(uv));
}

// The layer's graded, keyed and masked color, not yet premultiplied.
fn layer_color(in: VertexOutput) -> vec4<f32> {
    let pinned = unpinned(in.uv);
    let uv = warp(pinned);
    let color = source_color(uv);
    let wiped = wipe_cover(in.canvas);
    let keyed = key_alpha(color.rgb);
    // Desaturating the key's edge suppresses the screen's color spilling onto it.
    let unspilled = mix(vec3<f32>(dot(color.rgb, LUMA)), color.rgb, keyed);
    let sharp = sharpen(unspilled, uv);
    var rgb = restyle(stylize(secondary(creative_grade(correct(change_to_color(mix_channels(leave_color(sharp)))))), pinned), uv);
    rgb = mix(rgb, vec3<f32>(1.0), placement.balance.w);
    let picture = color.a * keyed * f32(in_crop(pinned));
    let shadow = shadow_alpha(pinned);
    let covered = picture + shadow * (1.0 - picture);
    rgb = mix(placement.style_shadow_color.rgb, rgb, picture / max(covered, 1e-6));
    let opacity = placement.axis_y_opacity.z * mask_coverage(in.uv);
    let shown = covered * wiped.picture;
    let border = 1.0 - wiped.picture;
    rgb = (rgb * shown + wiped.border * border) / max(shown + border, 1e-6);
    return vec4<f32>(rgb, (shown + border) * wiped.shown * opacity);
}
