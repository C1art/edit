// Color Balance (RGB), Solarize, Threshold, Replicate, Luma Key and Alpha Adjust, which
// work on encoded levels as Premiere's effects do.

fn replicated(uv: vec2<f32>) -> vec2<f32> {
    let count = placement.adjust_balance.w;
    if count <= 1.0 {
        return uv;
    }
    return fract(uv * floor(count));
}

fn adjusted(rgb: vec3<f32>) -> vec3<f32> {
    let levels = placement.adjust_levels;
    var encoded = to_gamma(max(rgb, vec3<f32>(0.0))) * placement.adjust_balance.rgb;
    if levels.x >= 0.0 {
        encoded = select(encoded, 1.0 - encoded, encoded > vec3<f32>(levels.x));
    }
    if levels.y >= 0.0 {
        encoded = vec3<f32>(step(levels.y, dot(encoded, LUMA)));
    }
    return to_linear(encoded);
}

// Opaque at the threshold's luma and beyond it, away from the cutoff.
fn luma_matte(rgb: vec3<f32>) -> f32 {
    let threshold = placement.adjust_levels.z;
    if threshold < 0.0 {
        return 1.0;
    }
    let cutoff = placement.adjust_levels.w;
    let luma = dot(to_gamma(max(rgb, vec3<f32>(0.0))), LUMA);
    if abs(threshold - cutoff) < 1e-4 {
        return step(threshold, luma);
    }
    return saturate((luma - cutoff) / (threshold - cutoff));
}

fn adjusted_alpha(alpha: f32) -> f32 {
    let adjust = placement.adjust_alpha;
    let read = select(alpha, 1.0, adjust.y > 0.5);
    return select(read, 1.0 - read, adjust.z > 0.5) * adjust.x;
}

// With Mask Only, the coverage in grey levels and opaque.
fn alpha_view(rgb: vec3<f32>, coverage: f32) -> vec4<f32> {
    if placement.adjust_alpha.w > 0.5 {
        return vec4<f32>(to_linear(vec3<f32>(coverage)), 1.0);
    }
    return vec4<f32>(rgb, coverage);
}
