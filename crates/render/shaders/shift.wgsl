fn change_color(rgb: vec3<f32>) -> vec3<f32> {
    let target_color = placement.shift_change_target;
    if target_color.w < 0.0 {
        return rgb;
    }
    let amounts = placement.shift_change_amounts;
    let detail = placement.shift_change_detail;
    let encoded = to_gamma(max(rgb, vec3<f32>(0.0)));
    var distance = length(encoded - target_color.rgb) / sqrt(3.0);
    if target_color.w > 0.5 && target_color.w < 1.5 {
        let turn = abs(hsl(rgb).x - hsl(to_linear(target_color.rgb)).x);
        distance = min(turn, 1.0 - turn) * 2.0;
    } else if target_color.w > 1.5 {
        let chroma = encoded - vec3<f32>(dot(encoded, LUMA));
        let wanted = target_color.rgb - vec3<f32>(dot(target_color.rgb, LUMA));
        distance = length(chroma.rb - wanted.rb);
    }
    var weight = 1.0 - smoothstep(amounts.w, amounts.w + detail.x + 1e-4, distance);
    if detail.y > 0.5 {
        weight = 1.0 - weight;
    }
    if detail.z > 0.5 {
        return to_linear(vec3<f32>(weight));
    }
    if weight <= 0.0 {
        return rgb;
    }
    let color = hsl(rgb);
    let changed = vec3<f32>(
        fract(color.x + amounts.x),
        clamp(color.y * (1.0 + amounts.z), 0.0, 1.0),
        clamp(color.z + amounts.y * select(color.z, 1.0 - color.z, amounts.y > 0.0), 0.0, 1.0),
    );
    return to_linear(mix(encoded, from_hsl(changed), weight));
}

fn color_balance(rgb: vec3<f32>) -> vec3<f32> {
    let shadows = placement.shift_balance_shadows;
    if shadows.w < 0.5 {
        return rgb;
    }
    let midtones = placement.shift_balance_midtones;
    let highlights = placement.shift_balance_highlights;
    let encoded = clamp(to_gamma(max(rgb, vec3<f32>(0.0))), vec3<f32>(0.0), vec3<f32>(1.0));
    let luma = dot(encoded, LUMA);
    let in_shadows = 1.0 - smoothstep(0.0, 0.5, luma);
    let in_highlights = smoothstep(0.5, 1.0, luma);
    let in_midtones = 1.0 - in_shadows - in_highlights;
    let push = shadows.rgb * in_shadows + midtones.rgb * in_midtones
        + highlights.rgb * in_highlights;
    var balanced = clamp(encoded + push * 0.5, vec3<f32>(0.0), vec3<f32>(1.0));
    if midtones.w > 0.5 {
        balanced = clamp(balanced + vec3<f32>(luma - dot(balanced, LUMA)), vec3<f32>(0.0), vec3<f32>(1.0));
    }
    return to_linear(balanced);
}
