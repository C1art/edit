// Checkerboard, Grid, Circle and 4-Color Gradient, which draw over the picture as
// Premiere's Generate effects do; their colors are gamma-encoded.

// `fill` over `color` where `cover` of it shows, which fills even where it was clear.
fn drawn_over(color: vec4<f32>, fill: vec3<f32>, cover: f32) -> vec4<f32> {
    return vec4<f32>(mix(color.rgb, srgb_decode(fill), cover), mix(color.a, 1.0, cover));
}

fn checkerboard(color: vec4<f32>, p: vec2<f32>, picture: vec2<f32>) -> vec4<f32> {
    let board = placement.generate_checker;
    let fill = placement.generate_checker_color;
    if fill.w <= 0.0 {
        return color;
    }
    let cell = vec2<i32>(floor((p - board.xy * picture) / board.zw));
    if ((cell.x + cell.y) & 1) != 0 {
        return color;
    }
    return drawn_over(color, fill.rgb, fill.w);
}

fn grid(color: vec4<f32>, p: vec2<f32>, picture: vec2<f32>) -> vec4<f32> {
    let cells = placement.generate_grid;
    let fill = placement.generate_grid_color;
    if fill.w <= 0.0 {
        return color;
    }
    let detail = placement.generate_detail;
    // Pixels to the nearest line, which runs along the cells' edges.
    let away = abs(fract((p - cells.xy * picture) / cells.zw + 0.5) - 0.5) * cells.zw;
    let half = detail.x * 0.5;
    var line = 1.0 - smoothstep(half - 0.5, half + 0.5, min(away.x, away.y));
    if detail.y > 0.5 {
        line = 1.0 - line;
    }
    return drawn_over(color, fill.rgb, fill.w * line);
}

fn circle(color: vec4<f32>, p: vec2<f32>, picture: vec2<f32>) -> vec4<f32> {
    let shape = placement.generate_circle;
    let fill = placement.generate_circle_color;
    if fill.w <= 0.0 {
        return color;
    }
    let away = distance(p, shape.xy * picture);
    var inside = 1.0 - smoothstep(shape.z - 0.5, shape.z + 0.5 + shape.w, away);
    if placement.generate_detail.z > 0.5 {
        inside = 1.0 - inside;
    }
    return drawn_over(color, fill.rgb, fill.w * inside);
}

fn four_color_gradient(color: vec4<f32>, uv: vec2<f32>, picture: vec2<f32>) -> vec4<f32> {
    let colors = placement.generate_four_colors;
    let opacity = colors[0].w;
    if opacity <= 0.0 {
        return color;
    }
    let corners = placement.generate_four_points;
    let points = array<vec2<f32>, 4>(corners[0].xy, corners[0].zw, corners[1].xy, corners[1].zw);
    let aspect = picture / max(picture.x, picture.y);
    var distances: array<f32, 4>;
    var nearest = 1e9;
    for (var i = 0; i < 4; i++) {
        distances[i] = max(length((uv - points[i]) * aspect), 1e-5);
        nearest = min(nearest, distances[i]);
    }
    // Weights relative to the nearest point's stay finite however sharp the falloff.
    let falloff = 2.0 / placement.generate_detail.w;
    var total = 0.0;
    var mixed = vec3<f32>(0.0);
    for (var i = 0; i < 4; i++) {
        let weight = pow(nearest / distances[i], falloff);
        total += weight;
        mixed += colors[i].rgb * weight;
    }
    return drawn_over(color, mixed / total, opacity);
}

fn generated(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) {
        return color;
    }
    let picture = picture_size();
    let p = uv * picture;
    let gradient = four_color_gradient(color, uv, picture);
    return circle(grid(checkerboard(gradient, p, picture), p, picture), p, picture);
}
