const DETAIL_TAPS: i32 = 16;

// How far apart two colors' encoded lumas are, as the effects' 0-255 thresholds measure.
fn encoded_distance(a: vec3<f32>, b: vec3<f32>) -> f32 {
    let zero = vec3<f32>(0.0);
    return abs(dot(to_gamma(max(a, zero)) - to_gamma(max(b, zero)), LUMA));
}

fn graded_at(uv: vec2<f32>) -> vec3<f32> {
    return graded(textureSampleLevel(source, source_sampler, uv, 0.0));
}

// A neighbouring texel brought to the working space as `source_color` brings the
// pixel itself, so an HDR or premultiplied picture's neighbours compare like for like.
fn graded(color: vec4<f32>) -> vec3<f32> {
    return input_grade(color_managed(unpremultiplied(color)));
}

// Unsharp Mask: adds back the difference from a disc blur where it passes the
// threshold, so flat areas don't pick up their noise.
fn unsharp(rgb: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let settings = placement.detail_unsharp;
    if settings.x <= 0.0 || settings.y <= 0.0 {
        return rgb;
    }
    // #if detail_unsharp.x detail_unsharp.y
    let reach = settings.y / picture_size();
    var sum = vec3<f32>(0.0);
    for (var tap = 0; tap < DETAIL_TAPS; tap++) {
        let radius = sqrt((f32(tap) + 0.5) / f32(DETAIL_TAPS));
        let angle = f32(tap) * GOLDEN_ANGLE;
        sum += graded_at(uv + vec2<f32>(cos(angle), sin(angle)) * radius * reach);
    }
    let blurred = sum / f32(DETAIL_TAPS);
    let passes = step(settings.z, encoded_distance(rgb, blurred));
    return max(rgb + settings.x * (rgb - blurred) * passes, vec3<f32>(0.0));
    // #else
    return rgb;
    // #endif
}

fn sorted_pair(a: ptr<function, vec3<f32>>, b: ptr<function, vec3<f32>>) {
    let low = min(*a, *b);
    *b = max(*a, *b);
    *a = low;
}

// Median and Dust & Scratches: the median of the 3x3 neighbors `radius` pixels apart,
// channel by channel, through the classic nineteen-swap network.
fn median_filtered(rgb: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let radius = placement.detail_unsharp.w;
    if radius <= 0.0 {
        return rgb;
    }
    // #if detail_unsharp.w
    let step = radius / picture_size();
    var p0 = graded_at(uv + vec2<f32>(-step.x, -step.y));
    var p1 = graded_at(uv + vec2<f32>(0.0, -step.y));
    var p2 = graded_at(uv + vec2<f32>(step.x, -step.y));
    var p3 = graded_at(uv + vec2<f32>(-step.x, 0.0));
    var p4 = rgb;
    var p5 = graded_at(uv + vec2<f32>(step.x, 0.0));
    var p6 = graded_at(uv + vec2<f32>(-step.x, step.y));
    var p7 = graded_at(uv + vec2<f32>(0.0, step.y));
    var p8 = graded_at(uv + vec2<f32>(step.x, step.y));
    sorted_pair(&p1, &p2); sorted_pair(&p4, &p5); sorted_pair(&p7, &p8);
    sorted_pair(&p0, &p1); sorted_pair(&p3, &p4); sorted_pair(&p6, &p7);
    sorted_pair(&p1, &p2); sorted_pair(&p4, &p5); sorted_pair(&p7, &p8);
    sorted_pair(&p0, &p3); sorted_pair(&p5, &p8); sorted_pair(&p4, &p7);
    sorted_pair(&p3, &p6); sorted_pair(&p1, &p4); sorted_pair(&p2, &p5);
    sorted_pair(&p4, &p7); sorted_pair(&p4, &p2); sorted_pair(&p6, &p4);
    sorted_pair(&p4, &p2);
    return select(rgb, p4, encoded_distance(rgb, p4) > placement.detail_median.x);
    // #else
    return rgb;
    // #endif
}

// The picture's opacity fading toward its edges, Edge Feather's pixels in from each.
fn edge_feathered(uv: vec2<f32>) -> f32 {
    let feather = placement.detail_median.y;
    if feather <= 0.0 {
        return 1.0;
    }
    let inside = min(uv, 1.0 - uv) * picture_size();
    return saturate(min(inside.x, inside.y) / feather);
}

fn detail_hash(cell: vec2<f32>, salt: f32) -> f32 {
    return fract(sin(dot(cell, vec2<f32>(12.9898, 78.233)) + salt * 37.719) * 43758.5453) * 2.0 - 1.0;
}

// Noise HLS: each grain shifts its hue, lightness and saturation at random. The phase
// fades between two patterns, so turning it animates the noise smoothly.
fn noise_hls(rgb: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let amounts = placement.detail_noise.xyz;
    if all(amounts == vec3<f32>(0.0)) {
        return rgb;
    }
    // #if detail_noise
    let cell = floor(uv * picture_size() / placement.detail_median.z);
    let phase = placement.detail_median.w / TAU;
    let turn = floor(phase);
    let blend = phase - turn;
    let noise = vec3<f32>(
        mix(detail_hash(cell, turn), detail_hash(cell, turn + 1.0), blend),
        mix(detail_hash(cell, turn + 0.31), detail_hash(cell, turn + 1.31), blend),
        mix(detail_hash(cell, turn + 0.67), detail_hash(cell, turn + 1.67), blend),
    ) * amounts;
    let encoded = to_gamma(max(rgb, vec3<f32>(0.0)));
    let axis = vec3<f32>(0.57735027);
    let angle = noise.x * TAU;
    let turned = encoded * cos(angle) + cross(axis, encoded) * sin(angle)
        + axis * dot(axis, encoded) * (1.0 - cos(angle));
    let luma = dot(turned, LUMA);
    let shifted = vec3<f32>(luma) + (turned - luma) * (1.0 + noise.z) + noise.y;
    return to_linear(max(shifted, vec3<f32>(0.0)));
    // #else
    return rgb;
    // #endif
}
