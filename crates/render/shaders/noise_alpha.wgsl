// Noise Alpha: random opacity per picture pixel, mixed into the picture's own.

fn noise_alpha(alpha: f32, uv: vec2<f32>) -> f32 {
    let look = placement.noise_alpha;
    let timing = placement.noise_alpha_timing;
    if timing.w < 0.5 {
        return alpha;
    }
    // #if noise_alpha_timing.w
    let cell = floor(uv * picture_size());
    var noise = detail_hash(cell, timing.x) * 0.5 + 0.5;
    if timing.w > 1.5 {
        var now = floor(timing.y);
        let blend = timing.y - now;
        var next = now + 1.0;
        if timing.z > 0.0 {
            now -= timing.z * floor(now / timing.z);
            next -= timing.z * floor(next / timing.z);
        }
        noise = mix(
            detail_hash(cell, timing.x + now),
            detail_hash(cell, timing.x + next),
            blend,
        ) * 0.5 + 0.5;
    }
    if look.y > 0.5 {
        noise *= noise;
    }
    let added = noise * look.x;
    var noisy = alpha + added;
    if look.z > 2.5 {
        noisy = alpha + added * (1.0 - alpha);
    } else if look.z > 1.5 {
        noisy = alpha + added * alpha;
    } else if look.z > 0.5 {
        noisy = alpha * (1.0 - look.x + added);
    }
    if look.w > 1.5 {
        return select(noisy - floor(noisy), noisy, noisy >= 0.0 && noisy <= 1.0);
    }
    if look.w > 0.5 {
        let folded = noisy - 2.0 * floor(noisy * 0.5);
        return select(folded, 2.0 - folded, folded > 1.0);
    }
    return clamp(noisy, 0.0, 1.0);
    // #else
    return alpha;
    // #endif
}
