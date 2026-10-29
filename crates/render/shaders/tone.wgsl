// Convolution Kernel: the weighted 3x3 neighborhood one pixel apart, in encoded levels.
fn convolved(rgb: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let a = placement.tone_kernel_a;
    let b = placement.tone_kernel_b;
    let c = placement.tone_kernel_c;
    if c.z < 0.5 {
        return rgb;
    }
    // #if tone_kernel_c.z
    var weights = array<f32, 9>(a.x, a.y, a.z, a.w, b.x, b.y, b.z, b.w, c.x);
    let step = 1.0 / picture_size();
    var sum = vec3<f32>(0.0);
    for (var tap = 0; tap < 9; tap++) {
        let offset = vec2<f32>(f32(tap % 3 - 1), f32(tap / 3 - 1)) * step;
        var sample = rgb;
        if tap != 4 {
            sample = graded_at(uv + offset);
        }
        sum += weights[tap] * to_gamma(sample);
    }
    return to_linear(clamp(sum + c.y, vec3<f32>(0.0), vec3<f32>(1.0)));
    // #else
    return rgb;
    // #endif
}

// Shadow/Highlight: the surroundings' encoded luma decides how far a pixel is lifted
// out of the shadows or pulled down from the highlights, so that local contrast stays.
fn shadow_highlight(rgb: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let amounts = placement.tone_lift;
    let more = placement.tone_lift_more;
    if more.z >= 1.0 {
        return rgb;
    }
    // #if tone_lift_more.z
    let reach = max(more.x, 1.0) / picture_size();
    var sum = 0.0;
    for (var tap = 0; tap < DETAIL_TAPS; tap++) {
        let radius = sqrt((f32(tap) + 0.5) / f32(DETAIL_TAPS));
        let angle = f32(tap) * GOLDEN_ANGLE;
        let around = uv + vec2<f32>(cos(angle), sin(angle)) * radius * reach;
        sum += dot(to_gamma(graded_at(around)), LUMA);
    }
    let around = sum / f32(DETAIL_TAPS);
    let encoded = to_gamma(rgb);
    let shadows = 1.0 - smoothstep(0.0, max(amounts.z, 1e-3), around);
    let highlights = smoothstep(1.0 - max(amounts.w, 1e-3), 1.0, around);
    var toned = pow(encoded, vec3<f32>(1.0 / (1.0 + 2.0 * amounts.x * shadows)));
    if amounts.y > 0.0 {
        let room = max(vec3<f32>(1.0) - toned, vec3<f32>(0.0));
        toned = 1.0 - pow(room, vec3<f32>(1.0 / (1.0 + 2.0 * amounts.y * highlights)));
    }
    let luma = dot(toned, LUMA);
    let midtones = clamp(1.0 - abs(2.0 * luma - 1.0), 0.0, 1.0);
    toned = mix(toned, (toned - 0.5) * (1.0 + more.y) + 0.5, midtones);
    return to_linear(mix(toned, encoded, more.z));
    // #else
    return rgb;
    // #endif
}

// ASC CDL: slope, offset and power per channel on encoded levels, then saturation
// about Rec. 709 luma.
fn cdl_graded(rgb: vec3<f32>) -> vec3<f32> {
    let slope = placement.tone_cdl_slope;
    let offset = placement.tone_cdl_offset;
    if offset.w < 0.5 {
        return rgb;
    }
    // #if tone_cdl_offset.w
    let sloped = clamp(to_gamma(rgb) * slope.xyz + offset.xyz, vec3<f32>(0.0), vec3<f32>(1.0));
    let graded = pow(sloped, placement.tone_cdl_power.xyz);
    let luma = dot(graded, LUMA);
    return to_linear(luma + slope.w * (graded - luma));
    // #else
    return rgb;
    // #endif
}

// Color Replace: colors within the similarity of the target take on the replacement,
// shifted by as much as the target is so that their shading stays, or flat when solid.
fn color_replaced(rgb: vec3<f32>) -> vec3<f32> {
    let target_color = placement.tone_replace_target;
    let replacement = placement.tone_replace_with;
    if replacement.w == 0.0 {
        return rgb;
    }
    // #if tone_replace_with.w
    let encoded = to_gamma(rgb);
    let distance = length(encoded - target_color.rgb) / sqrt(3.0);
    let similarity = max(target_color.w, 1e-4);
    let near = 1.0 - smoothstep(similarity * 0.8, similarity, distance);
    var replaced = clamp(encoded + replacement.rgb - target_color.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    if replacement.w > 1.5 {
        replaced = replacement.rgb;
    }
    return to_linear(mix(encoded, replaced, near));
    // #else
    return rgb;
    // #endif
}

// Extract: white where the encoded luma lies between the levels, black elsewhere.
fn extracted(rgb: vec3<f32>) -> vec3<f32> {
    let levels = placement.tone_extract;
    if levels.w == 0.0 {
        return rgb;
    }
    // #if tone_extract.w
    let luma = dot(to_gamma(rgb), LUMA);
    let soft = max(levels.z, 1e-4);
    var inside = smoothstep(levels.x - soft, levels.x, luma)
        * (1.0 - smoothstep(levels.y, levels.y + soft, luma));
    if levels.w < 0.0 {
        inside = 1.0 - inside;
    }
    return to_linear(vec3<f32>(inside));
    // #else
    return rgb;
    // #endif
}

fn toned(rgb: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    return extracted(color_replaced(cdl_graded(shadow_highlight(rgb, uv))));
}

// Video Limiter: eases the brightest channel into the clip level past the knee, by
// taking out chroma, luma or both.
fn limited(rgb: vec3<f32>) -> vec3<f32> {
    let limit = placement.tone_limit;
    if limit.z == 0.0 {
        return rgb;
    }
    // #if tone_limit.z
    let encoded = to_gamma(rgb);
    let peak = max(encoded.r, max(encoded.g, encoded.b));
    let knee = limit.x * (1.0 - limit.y);
    let room = max(limit.x - knee, 1e-4);
    if peak <= knee {
        return rgb;
    }
    let eased = knee + room * (1.0 - exp(-(peak - knee) / room));
    let warning = placement.tone_limit_warning;
    if warning.w > 0.5 {
        return select(rgb, warning.rgb, peak > limit.x);
    }
    let luma = dot(encoded, LUMA);
    let chroma = encoded - luma;
    let top = max(peak - luma, 1e-5);
    var keep = 1.0;
    switch i32(limit.z) {
        case 2, 4: {
            keep = clamp((eased - luma) / top, 0.0, 1.0);
        }
        case 3: {
            keep = clamp((peak - (peak - eased) * 0.5 - luma) / top, 0.0, 1.0);
        }
        default: {}
    }
    if i32(limit.z) == 4 {
        keep = max(keep, 0.5);
    }
    var out = luma + chroma * keep;
    if i32(limit.z) != 2 {
        out -= max(luma + top * keep - eased, 0.0);
    }
    return to_linear(out);
    // #else
    return rgb;
    // #endif
}
