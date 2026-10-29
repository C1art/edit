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
    // #if creative_shadow creative_highlight
    return split_tone(faded(rgb));
    // #else
    return rgb;
    // #endif
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
    // #if creative_vignette.x
    let distance = vignette_distance(uv, vignette.z);
    let edge = smoothstep(vignette.y, vignette.y + max(vignette.w, 0.01), distance);
    if vignette.x < 0.0 {
        return rgb * (1.0 + vignette.x * edge);
    }
    return mix(rgb, vec3<f32>(1.0), vignette.x * edge);
    // #else
    return rgb;
    // #endif
}

// Gamma-encoded color from hue in turns, saturation and lightness.
fn hsl_color(color: vec3<f32>) -> vec3<f32> {
    let chroma = (1.0 - abs(2.0 * color.z - 1.0)) * color.y;
    let sector = vec3<f32>(0.0, 2.0 / 3.0, 1.0 / 3.0);
    let ramp = clamp(abs(fract(vec3<f32>(color.x) + sector) * 6.0 - 3.0) - 1.0, vec3<f32>(0.0), vec3<f32>(1.0));
    return color.z + chroma * (ramp - 0.5);
}

// Lumetri's Hue Saturation Curves, read from the rows below the RGB curves.
fn hue_curves(rgb: vec3<f32>) -> vec3<f32> {
    let size = vec2<f32>(textureDimensions(curves));
    var color = hsl(rgb);
    let along = vec3<f32>(color.x, color.z, color.y) * ((size.x - 1.0) / size.x) + vec3<f32>(0.5 / size.x);
    let by_hue = textureSampleLevel(curves, grade_sampler, vec2<f32>(along.x, 1.5 / size.y), 0.0).rgb;
    let by_luma = textureSampleLevel(curves, grade_sampler, vec2<f32>(along.y, 2.5 / size.y), 0.0).r;
    let by_saturation = textureSampleLevel(curves, grade_sampler, vec2<f32>(along.z, 2.5 / size.y), 0.0).g;
    // Greys have no hue to pick them out, so brightening by hue fades in with
    // saturation; scaling saturation and turning hue leave greys alone anyway.
    let hued = smoothstep(0.0, 0.25, color.y);
    color.x = fract(color.x + by_hue.y);
    color.z = clamp(color.z + by_hue.z * hued, 0.0, 1.0);
    color.y = clamp(color.y * by_hue.x * by_luma * by_saturation, 0.0, 1.0);
    return to_linear(hsl_color(color));
}
