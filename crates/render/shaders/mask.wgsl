// Opacity masks: ellipses, rectangles and pen paths, feathered and expanded.

// Signed distance outside an ellipse, close enough near its outline for a feather.
fn ellipse_distance(p: vec2<f32>, radii: vec2<f32>) -> f32 {
    let scaled = length(p / radii);
    let gradient = length(p / (radii * radii));
    if gradient < 1e-6 {
        return -min(radii.x, radii.y);
    }
    return scaled * (scaled - 1.0) / gradient;
}

fn box_distance(p: vec2<f32>, half_size: vec2<f32>) -> f32 {
    let q = abs(p) - half_size;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
}

fn outline_point(mask: u32, index: u32) -> vec2<f32> {
    let pair = placement.outlines[mask * OUTLINE_VEC4S + index / 2u];
    return select(pair.xy, pair.zw, index % 2u == 1u);
}

// Signed distance outside a mask's path outline in pixels, its sign from the crossings
// of a ray along +x.
fn path_distance(uv: vec2<f32>, mask: u32, count: u32, picture: vec2<f32>) -> f32 {
    let p = uv * picture;
    var nearest = 1e20;
    var side = 1.0;
    var previous = outline_point(mask, count - 1u) * picture;
    for (var i = 0u; i < count; i++) {
        let vertex = outline_point(mask, i) * picture;
        let edge = previous - vertex;
        let to_p = p - vertex;
        let along = clamp(dot(to_p, edge) / max(dot(edge, edge), 1e-6), 0.0, 1.0);
        let gap = to_p - edge * along;
        nearest = min(nearest, dot(gap, gap));
        let crossing = vec3<bool>(
            (p.y >= vertex.y),
            (p.y < previous.y),
            (edge.x * to_p.y > edge.y * to_p.x),
        );
        if all(crossing) || !any(crossing) {
            side = -side;
        }
        previous = vertex;
    }
    return side * sqrt(nearest);
}

// How much of the picture at `uv` the masks keep. Masks add: each covers what the
// others leave by its own opacity. Intersecting masks then cut away what lies outside
// them.
fn mask_coverage(uv: vec2<f32>) -> f32 {
    let count = u32(placement.mask_space.z);
    if count == 0u {
        return 1.0;
    }
    let picture = placement.mask_space.xy;
    var uncovered = 1.0;
    var adding = false;
    var kept = 1.0;
    for (var i = 0u; i < count; i++) {
        let shape = placement.masks[i * 3u];
        let edge = placement.masks[i * 3u + 1u];
        let kind = placement.masks[i * 3u + 2u];
        let offset = (uv - shape.xy) * picture;
        // Turning back by the mask's clockwise rotation, with +y down.
        let sin_cos = vec2<f32>(sin(edge.x), cos(edge.x));
        let p = vec2<f32>(
            sin_cos.y * offset.x + sin_cos.x * offset.y,
            -sin_cos.x * offset.x + sin_cos.y * offset.y,
        );
        let half_size = max(shape.zw * picture * 0.5, vec2<f32>(1e-3));
        var distance = ellipse_distance(p, half_size);
        if kind.x > 1.5 {
            distance = path_distance(uv, i, u32(kind.z), picture);
        } else if kind.x > 0.5 {
            distance = box_distance(p, half_size);
        }
        // A pixel's softness at least, so a hard edge doesn't alias.
        let feather = max(edge.y, 1.0);
        var inside = 1.0 - smoothstep(-0.5 * feather, 0.5 * feather, distance - edge.z);
        if kind.y > 0.5 {
            inside = 1.0 - inside;
        }
        if kind.w > 0.5 {
            kept *= 1.0 - (1.0 - inside) * edge.w;
        } else {
            uncovered *= 1.0 - inside * edge.w;
            adding = true;
        }
    }
    return select(1.0, 1.0 - uncovered, adding) * kept;
}
