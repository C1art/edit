// Ellipse and Cell Pattern, which draw over the picture as Premiere's Generate effects
// do; their colors are gamma-encoded.

fn ellipse_ring(color: vec4<f32>, p: vec2<f32>, picture: vec2<f32>) -> vec4<f32> {
    let detail = placement.pattern_ellipse_detail;
    if detail.w < 0.5 {
        return color;
    }
    let shape = placement.pattern_ellipse;
    let radii = max(shape.zw * 0.5, vec2<f32>(1.0));
    // Pixels from the outline, close enough for a ring that is not too eccentric.
    let away = (length((p - shape.xy * picture) / radii) - 1.0) * min(radii.x, radii.y);
    let half = detail.x * 0.5;
    let soft = detail.y * half;
    let cover = 1.0 - smoothstep(half - soft - 0.5, half + 0.5, abs(away));
    let across = clamp(away / max(detail.x, 1e-3) + 0.5, 0.0, 1.0);
    let fill = mix(placement.pattern_ellipse_inside.rgb, placement.pattern_ellipse_outside.rgb, across);
    var under = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if detail.z > 0.5 {
        under = color;
    }
    return drawn_over(under, fill, cover);
}

fn patterned(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return color;
    }
    let picture = picture_size();
    let p = uv * picture;
    return ellipse_ring(cell_pattern(color, p), p, picture);
}

fn cell_hash(cell: vec2<f32>) -> vec2<f32> {
    return fract(sin(vec2<f32>(dot(cell, vec2<f32>(127.1, 311.7)), dot(cell, vec2<f32>(269.5, 183.3))))
        * 43758.5453);
}

fn cell_pattern(color: vec4<f32>, p: vec2<f32>) -> vec4<f32> {
    let cells = placement.pattern_cells;
    if cells.x < 0.0 {
        return color;
    }
    let more = placement.pattern_cells_more;
    let at = (p - more.xy) / cells.y;
    let home = floor(at);
    var nearest = 9.0;
    var second = 9.0;
    var owner = vec2<f32>(0.0);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let cell = home + vec2<f32>(f32(x), f32(y));
            let seed = cell_hash(cell);
            // Each center circles its own spot as the evolution turns.
            let turn = (more.z + seed.x) * 6.2831853;
            let center = cell + 0.5 + (seed - 0.5) * cells.z
                + vec2<f32>(cos(turn), sin(turn)) * 0.15 * cells.z;
            let d = distance(at, center);
            if d < nearest {
                second = nearest;
                nearest = d;
                owner = cell;
            } else if d < second {
                second = d;
            }
        }
    }
    let shade = cell_hash(owner + 17.0).x;
    var value = 0.0;
    switch i32(cells.x + 0.5) {
        case 0: { value = 1.0 - nearest; }
        case 1: { value = clamp((second - nearest) * 2.0, 0.0, 1.0); }
        case 2: { value = fract(shade + more.z); }
        case 3: { value = shade; }
        case 4: { value = smoothstep(0.0, 0.08, second - nearest) * shade; }
        default: { value = pow(clamp(1.0 - nearest, 0.0, 1.0), 2.0); }
    }
    value = clamp((value - 0.5) * cells.w + 0.5, 0.0, 1.0);
    if placement.pattern_cells_flags.x > 0.5 {
        value = 1.0 - value;
    }
    return drawn_over(color, vec3<f32>(value), 1.0 - more.w);
}
