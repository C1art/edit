// Ultra Key: a chroma matte, cleaned up, with the key's color taken out of what remains.

// The largest chroma distance between saturated colors is about this.
const CHROMA_RANGE: f32 = 0.5;
const KEY_TAPS: i32 = 8;
// Ultra Key's outputs, in declaration order.
const KEY_ALPHA_CHANNEL: f32 = 1.0;
const KEY_COLOR_CHANNEL: f32 = 2.0;

// BT.709 Cb and Cr of gamma-encoded color, so keying matches what the eye separates.
fn chroma(rgb: vec3<f32>) -> vec2<f32> {
    let c = to_gamma(rgb);
    return vec2<f32>(dot(c, vec3<f32>(-0.1146, -0.3854, 0.5)), dot(c, vec3<f32>(0.5, -0.4542, -0.0458)));
}

fn chroma_matte(rgb: vec3<f32>) -> f32 {
    let key = placement.key;
    let distance = length(chroma(rgb) - chroma(key.rgb));
    let tolerance = key.w * CHROMA_RANGE;
    let softness = max(placement.amounts.x * CHROMA_RANGE, 0.0001);
    return smoothstep(tolerance, tolerance + softness, distance);
}

// The matte shrunk by the choke, its edge blurred by the soften. Outside the picture the
// source reads as black, which is never keyed, so the frame's own edge isn't choked.
fn cleaned_matte(rgb: vec3<f32>, uv: vec2<f32>) -> f32 {
    let matte = chroma_matte(rgb);
    let cleanup = placement.key_matte;
    if cleanup.z <= 0.0 && cleanup.w <= 0.0 {
        return matte;
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(source));
    var eroded = matte;
    var blurred = matte;
    for (var tap = 0; tap < KEY_TAPS; tap++) {
        let angle = f32(tap) * 2.0 * PI / f32(KEY_TAPS);
        let toward = vec2<f32>(cos(angle), sin(angle)) * texel;
        eroded = min(eroded, chroma_matte(source_color(uv + toward * cleanup.z).rgb));
        blurred += chroma_matte(source_color(uv + toward * cleanup.w).rgb);
    }
    blurred /= f32(KEY_TAPS + 1);
    return select(blurred, min(eroded, blurred), cleanup.z > 0.0);
}

// How much of the source the key leaves, Color Key's included.
fn key_alpha(rgb: vec3<f32>, uv: vec2<f32>) -> f32 {
    if placement.key.w < 0.0 {
        return color_key_alpha(rgb);
    }
    let cleanup = placement.key_matte;
    var matte = cleaned_matte(rgb, uv);
    matte = saturate((matte - cleanup.x) / max(1.0 - cleanup.x, 1e-4));
    matte = saturate((matte - 0.5) * (1.0 + 4.0 * cleanup.y) + 0.5);
    return matte * color_key_alpha(rgb);
}

// The coverage the layer draws with: the views of the matte and the color are opaque.
fn key_shown(matte: f32) -> f32 {
    return select(matte, 1.0, placement.key_spill.y >= KEY_ALPHA_CHANNEL);
}

// The graded color, or in the alpha view the matte in grey levels.
fn key_view(rgb: vec3<f32>, matte: f32) -> vec3<f32> {
    let view = placement.key_spill.y;
    let showing_alpha = view >= KEY_ALPHA_CHANNEL && view < KEY_COLOR_CHANNEL;
    return select(rgb, to_linear(vec3<f32>(matte)), showing_alpha);
}

// Takes the key's hue out of the color and desaturates the matte's edge, where the
// screen's light spills most. The default half reaches full desaturation.
fn despill(rgb: vec3<f32>, matte: f32) -> vec3<f32> {
    let spill = placement.key_spill.x;
    if placement.key.w < 0.0 || spill <= 0.0 {
        return rgb;
    }
    let key = chroma(placement.key.rgb);
    let toward_key = key / max(length(key), 1e-4);
    var cbcr = chroma(rgb);
    cbcr -= toward_key * max(dot(cbcr, toward_key), 0.0) * spill;
    let y = dot(to_gamma(rgb), LUMA);
    let encoded = vec3<f32>(y + 1.5748 * cbcr.y, y - 0.1873 * cbcr.x - 0.4681 * cbcr.y, y + 1.8556 * cbcr.x);
    let despilled = to_linear(encoded);
    return mix(vec3<f32>(dot(despilled, LUMA)), despilled, mix(1.0, matte, min(spill * 2.0, 1.0)));
}
