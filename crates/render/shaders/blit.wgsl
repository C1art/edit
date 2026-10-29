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
    // width as a share of the canvas height and its color, linear; then its softness.
    wipes: array<vec4<f32>, 6>,
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
    // 1 rectangle, 2 path), 1 when inverted, a path's point count and 1 when it
    // intersects.
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
    // first row ending in 1 when on; the warped picture's bounds; then a page fold's
    // corner, progress, and 1 when flat or 2 when curling (see reshape.wgsl).
    reshape_mirror: vec4<f32>,
    reshape_unpin: array<vec4<f32>, 3>,
    reshape_bounds: vec4<f32>,
    reshape_fold: vec4<f32>,
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
    // Ultra Key's pedestal, contrast, choke and soften in source pixels; then its spill,
    // its output (see key.wgsl), and 1 to ignore the source's alpha or 2 to invert it.
    key_matte: vec4<f32>,
    key_spill: vec4<f32>,
    // Color Balance (RGB)'s gains, then Replicate's count; Solarize's and Threshold's
    // levels and Luma Key's threshold and cutoff, each negative when off; then Alpha
    // Adjust's opacity and 1 to ignore, to invert and to show only the alpha (see
    // adjust.wgsl). Levels' input black and white and output black and white, ProcAmp's
    // brightness, contrast, hue in radians and saturation, then Levels' gamma.
    adjust_balance: vec4<f32>,
    adjust_levels: vec4<f32>,
    adjust_alpha: vec4<f32>,
    adjust_range: vec4<f32>,
    adjust_proc_amp: vec4<f32>,
    adjust_gamma: vec4<f32>,
    // Lumetri's shadow tint toward blue and red, Faded Film and the tint balance; the
    // highlight tint; then the vignette's amount, midpoint, roundness and feather (see
    // lumetri.wgsl).
    creative_shadow: vec4<f32>,
    creative_highlight: vec4<f32>,
    creative_vignette: vec4<f32>,
    // Checkerboard's anchor and square size, then its encoded color and opacity, off
    // at zero; Grid's the same; Circle's center, radius and feather, then its color;
    // Grid's border and 1 when inverted, 1 when Circle is inverted and 4-Color
    // Gradient's blend; then that gradient's points and colors, the first carrying
    // its opacity (see generate.wgsl).
    generate_checker: vec4<f32>,
    generate_checker_color: vec4<f32>,
    generate_grid: vec4<f32>,
    generate_grid_color: vec4<f32>,
    generate_circle: vec4<f32>,
    generate_circle_color: vec4<f32>,
    generate_detail: vec4<f32>,
    generate_four_points: array<vec4<f32>, 2>,
    generate_four_colors: array<vec4<f32>, 4>,
    // Lens Flare's center, brightness and lens; Magnify's center, magnification and
    // size; then Magnify's feather, 1 when square and opacity, and how much of the
    // flare is left out (see optics.wgsl).
    optics_flare: vec4<f32>,
    optics_magnify: vec4<f32>,
    optics_detail: vec4<f32>,
    // Linear Wipe's completion, angle in radians and feather; Radial Wipe's center,
    // completion and start angle, then its feather and which way it sweeps; Venetian
    // Blinds' completion, direction, width and feather; then Block Dissolve's
    // completion, block size and feather (see clip_wipes.wgsl).
    clip_linear: vec4<f32>,
    clip_radial: vec4<f32>,
    clip_radial_detail: vec4<f32>,
    clip_blinds: vec4<f32>,
    clip_blocks: vec4<f32>,
    // Unsharp Mask's amount, radius and threshold, then the median's radius; the
    // median's threshold, Edge Feather's width, Noise HLS's grain size and phase; then
    // its hue, lightness and saturation amounts (see detail.wgsl).
    detail_unsharp: vec4<f32>,
    detail_median: vec4<f32>,
    detail_noise: vec4<f32>,
    // Roughen Edges' border, softness, fractal scale and complexity; its evolution,
    // fractal influence and offset; its edge color and whether to show it. Bevel Edges'
    // thickness, light angle and intensity, then its light color, and Bevel Alpha's the
    // same. Alpha Glow's size, brightness and fade-out, its start and end colors; then
    // Color Emboss's direction, relief, contrast and share (see edges.wgsl).
    edges_roughen: vec4<f32>,
    edges_roughen_noise: vec4<f32>,
    edges_roughen_color: vec4<f32>,
    edges_bevel: vec4<f32>,
    edges_bevel_color: vec4<f32>,
    edges_bevel_alpha: vec4<f32>,
    edges_bevel_alpha_color: vec4<f32>,
    edges_glow: vec4<f32>,
    edges_glow_start: vec4<f32>,
    edges_glow_end: vec4<f32>,
    edges_emboss: vec4<f32>,
    tone_kernel_a: vec4<f32>,
    tone_kernel_b: vec4<f32>,
    tone_kernel_c: vec4<f32>,
    tone_extract: vec4<f32>,
    tone_lift: vec4<f32>,
    tone_lift_more: vec4<f32>,
    tone_limit: vec4<f32>,
    tone_limit_warning: vec4<f32>,
    tone_cdl_slope: vec4<f32>,
    tone_cdl_offset: vec4<f32>,
    tone_cdl_power: vec4<f32>,
    tone_replace_target: vec4<f32>,
    tone_replace_with: vec4<f32>,
    shift_change_target: vec4<f32>,
    shift_change_amounts: vec4<f32>,
    shift_change_detail: vec4<f32>,
    shift_balance_shadows: vec4<f32>,
    shift_balance_midtones: vec4<f32>,
    shift_balance_highlights: vec4<f32>,
    pattern_ellipse: vec4<f32>,
    pattern_ellipse_detail: vec4<f32>,
    pattern_ellipse_inside: vec4<f32>,
    pattern_ellipse_outside: vec4<f32>,
    pattern_cells: vec4<f32>,
    pattern_cells_more: vec4<f32>,
    pattern_cells_flags: vec4<f32>,
    lighting_lamps: array<vec4<f32>, 15>,
    lighting_ambience: vec4<f32>,
    lighting_surface: vec4<f32>,
    lighting_flags: vec4<f32>,
    // Radial Shadow's light, as a fraction of the picture, and scale, then whether it
    // hides the picture and tints by it.
    point_shadow: vec4<f32>,
    point_shadow_look: vec4<f32>,
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

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Where the fragment lies on the canvas: 0..1, from the top left.
    @location(1) canvas: vec2<f32>,
};

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> placement: Placement;
// Lookups in gamma-encoded values: curves as a row of outputs per channel followed by
// two rows of hue curves, then the input and look LUTs as cubes indexed by red, green
// and blue.
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
    // Waves, a shadow and a glow reach past the picture, so the quad grows to hold them.
    let shadow = placement.style_shadow;
    let reach = abs(placement.style_wave.x) + select(
        0.0,
        max(abs(shadow.x), abs(shadow.y)) + shadow.z,
        shadow.w > 0.0,
    ) + select(0.0, placement.edges_glow.x, placement.edges_glow.y > 0.0);
    let margin = vec2<f32>(reach) / picture_size();
    var low = placement.crop.xy - margin;
    var high = vec2<f32>(1.0) - placement.crop.zw + margin;
    if placement.reshape_unpin[0].w > 0.5 {
        low = min(low, placement.reshape_bounds.xy - margin);
        high = max(high, placement.reshape_bounds.zw + margin);
    }
    // A folded page's back can lie up to a picture beyond it.
    let folding = f32(placement.reshape_fold.w > 0.5);
    low -= folding;
    high += folding;
    let radial = placement.point_shadow;
    if radial.w > 0.5 {
        let spread = vec2<f32>(placement.style_shadow.z) / picture_size();
        low = min(low, radial.xy * (1.0 - radial.z) - spread);
        high = max(high, radial.xy + (vec2<f32>(1.0) - radial.xy) * radial.z + spread);
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
    rgb = creative_tone(rgb);
    let flags = u32(grade.w + 0.5);
    if (flags & 1u) != 0u {
        let size = f32(textureDimensions(curves).x);
        let row = 0.5 / f32(textureDimensions(curves).y);
        let u = srgb_encode(rgb) * ((size - 1.0) / size) + vec3<f32>(0.5 / size);
        rgb = srgb_decode(vec3<f32>(
            textureSampleLevel(curves, grade_sampler, vec2<f32>(u.r, row), 0.0).r,
            textureSampleLevel(curves, grade_sampler, vec2<f32>(u.g, row), 0.0).g,
            textureSampleLevel(curves, grade_sampler, vec2<f32>(u.b, row), 0.0).b,
        ));
    }
    if (flags & 2u) != 0u {
        rgb = hue_curves(rgb);
    }
    return rgb;
}

fn to_gamma(rgb: vec3<f32>) -> vec3<f32> {
    return pow(max(rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / GAMMA));
}

fn to_linear(rgb: vec3<f32>) -> vec3<f32> {
    return pow(max(rgb, vec3<f32>(0.0)), vec3<f32>(GAMMA));
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
    return creative_vignette(rgb, uv);
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
    var color = picture_at(magnified(uv));
    if placement.axis_y_opacity.w > 0.5 {
        color = vec4<f32>(color.rgb / max(color.a, 1e-6), color.a);
    }
    let drawn = patterned(generated(ramped(vec4<f32>(input_grade(color.rgb), color.a), uv), uv), uv);
    return flared(drawn, uv);
}

// How much of the cropped, keyed picture covers `uv`, which casts the shadow.
fn cast_alpha(uv: vec2<f32>) -> f32 {
    let source = warp(replicated(uv));
    let color = source_color(source);
    let keyed = key_shown(key_alpha(color.rgb, source)) * luma_matte(color.rgb);
    return adjusted_alpha(color.a) * keyed * f32(in_crop(uv));
}

// The layer's graded, keyed and masked color, not yet premultiplied.
fn layer_color(in: VertexOutput) -> vec4<f32> {
    let page = folded(in.uv);
    let pinned = unpinned(page.uv);
    let uv = warp(replicated(pinned));
    let color = source_color(uv);
    let wiped = wipe_cover(in.canvas);
    let matte = key_alpha(color.rgb, uv);
    let keyed = key_shown(matte);
    let sharp = unsharp(sharpen(median_filtered(convolved(despill(color.rgb, matte), uv), uv), uv), uv);
    var rgb = noise_hls(adjusted(restyle(stylize(secondary(creative_grade(correct(color_balance(change_color(change_to_color(mix_channels(leave_color(toned(sharp, uv))))))))), pinned), uv)), pinned);
    let rough = roughened(pinned);
    rgb = roughen_colored(edge_lit(lighting_effects(rgb, uv), pinned, uv), rough.y);
    rgb = page_side(key_view(limited(rgb), matte), page);
    rgb = mix(rgb, vec3<f32>(1.0), placement.balance.w);
    let viewed = alpha_view(rgb, adjusted_alpha(color.a) * keyed * luma_matte(color.rgb) * f32(in_crop(pinned)) * rough.x);
    rgb = viewed.rgb;
    let picture = viewed.a * (1.0 - placement.point_shadow_look.x);
    let shadow = shadow_alpha(pinned);
    let glow = alpha_glow(pinned);
    let backing = glow.a + shadow * (1.0 - glow.a);
    let backing_rgb = mix(shadow_tint(pinned), glow.rgb, glow.a / max(backing, 1e-6));
    let covered = picture + backing * (1.0 - picture);
    rgb = mix(backing_rgb, rgb, picture / max(covered, 1e-6));
    let opacity = placement.axis_y_opacity.z * mask_coverage(page.uv) * clip_wipes_shown(page.uv)
        * edge_feathered(pinned);
    let shown = covered * wiped.picture;
    let border = 1.0 - wiped.picture;
    rgb = (rgb * shown + wiped.border * border) / max(shown + border, 1e-6);
    let alpha = (shown + border) * wiped.shown * opacity;
    let under = page.shadow * placement.axis_y_opacity.z * (1.0 - alpha);
    return vec4<f32>(rgb * alpha / max(alpha + under, 1e-6), alpha + under);
}
