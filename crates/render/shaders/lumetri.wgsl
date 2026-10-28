// Lumetri's Creative tone and Vignette.

// Stops a tint at its rim pushes its range toward the hue.
const TINT_STOPS: f32 = 0.5;
// Where Faded Film at full strength takes pure black and white, encoded.
const FADED_BLACK: f32 = 0.15;
const FADED_WHITE: f32 = 0.9;

fn faded(rgb: vec3<f32>) -> vec3<f32> {
    let amount = placement.creative_shadow.z;
    if amount <= 0.0 {
        return rgb;
    }
    let encoded = to_gamma(rgb);
    let range = mix(vec2<f32>(0.0, 1.0), vec2<f32>(FADED_BLACK, FADED_WHITE), amount);
    return to_linear(range.x + encoded * (range.y - range.x));
}

// Tints the shadows and highlights apart, splitting them at a luma the balance moves.
fn split_tone(rgb: vec3<f32>) -> vec3<f32> {
    let shadow = placement.creative_shadow;
    let highlight = placement.creative_highlight;
    let split = 0.5 - 0.4 * shadow.w;
    let toward_highlight = smoothstep(split - 0.4, split + 0.4, dot(to_gamma(rgb), LUMA));
    let push = mix(shadow.xy, highlight.xy, toward_highlight);
    let hue = vec3<f32>(1.5748 * push.y, -0.1873 * push.x - 0.4681 * push.y, 1.8556 * push.x);
    return rgb * exp2(hue * TINT_STOPS);
}

fn creative_tone(rgb: vec3<f32>) -> vec3<f32> {
    return split_tone(faded(rgb));
}

fn superellipse_radius(p: vec2<f32>, power: f32) -> f32 {
    return pow(dot(pow(p, vec2<f32>(power)), vec2<f32>(1.0)), 1.0 / power);
}

// 0 at the center, 1 in the corners. A round vignette is circular on the picture, a
// square one follows its edges.
fn vignette_distance(uv: vec2<f32>, roundness: f32) -> f32 {
    let size = picture_size();
    let shape = mix(vec2<f32>(1.0), size / max(size.x, size.y), max(roundness, 0.0));
    let power = 2.0 + 8.0 * max(-roundness, 0.0);
    let p = abs(uv - vec2<f32>(0.5)) * 2.0 * shape;
    return superellipse_radius(p, power) / superellipse_radius(shape, power);
}

fn creative_vignette(rgb: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let vignette = placement.creative_vignette;
    if vignette.x == 0.0 {
        return rgb;
    }
    let distance = vignette_distance(uv, vignette.z);
    let edge = smoothstep(vignette.y, vignette.y + max(vignette.w, 0.01), distance);
    if vignette.x < 0.0 {
        return rgb * (1.0 + vignette.x * edge);
    }
    return mix(rgb, vec3<f32>(1.0), vignette.x * edge);
}
