// The Transition category's clip effects, each hiding more of the picture as its
// completion grows (see clip_wipes.rs).

// How much of a point `along` a run of `length` stays shown once `completion` of the
// run is hidden from its start, the edge softened over `feather`. The edge starts
// wholly before the run and ends wholly past it, so 0 and 1 show and hide all of it.
fn run_shown(along: f32, length: f32, completion: f32, feather: f32) -> f32 {
    let soft = max(feather, 1e-4);
    let edge = completion * (length + soft) - 0.5 * soft;
    return clamp((along - edge) / soft + 0.5, 0.0, 1.0);
}

// The unit vector `angle` radians clockwise from up, in picture space where y runs down.
fn heading(angle: f32) -> vec2<f32> {
    return vec2<f32>(sin(angle), -cos(angle));
}

fn linear_wipe_shown(at: vec2<f32>, size: vec2<f32>) -> f32 {
    let wipe = placement.clip_linear;
    if wipe.x <= 0.0 {
        return 1.0;
    }
    let way = heading(wipe.y);
    let half = 0.5 * dot(abs(way), size);
    // A pixel's soft edge keeps a hard wipe from stepping along its slant.
    return run_shown(dot(at - 0.5 * size, way) + half, 2.0 * half, wipe.x, max(wipe.z, 1.0));
}

fn radial_wipe_shown(at: vec2<f32>, size: vec2<f32>) -> f32 {
    let wipe = placement.clip_radial;
    let detail = placement.clip_radial_detail;
    if wipe.z <= 0.0 {
        return 1.0;
    }
    let away = at - wipe.xy * size;
    let reach = max(length(away), 1.0);
    var turned = atan2(away.x, -away.y) - wipe.w;
    turned -= TAU * floor(turned / TAU);
    var span = TAU;
    if detail.y > 1.5 {
        turned = min(turned, TAU - turned);
        span = 0.5 * TAU;
    } else if detail.y > 0.5 {
        turned = TAU - turned;
    }
    // Measured along the arc, so the feather keeps its width in pixels at any distance.
    return run_shown(turned * reach, span * reach, wipe.z, max(detail.x, 1.0));
}

fn blinds_shown(at: vec2<f32>) -> f32 {
    let blinds = placement.clip_blinds;
    if blinds.x <= 0.0 {
        return 1.0;
    }
    let along = dot(at, heading(blinds.y));
    let within = along - blinds.z * floor(along / blinds.z);
    return run_shown(within, blinds.z, blinds.x, min(max(blinds.w, 1.0), blinds.z));
}

// Each block hides once the completion passes its fixed random rank; the feather fades
// blocks out over a share of the ranks as wide as it is of the blocks.
fn blocks_shown(at: vec2<f32>) -> f32 {
    let blocks = placement.clip_blocks;
    if blocks.x <= 0.0 {
        return 1.0;
    }
    let cell = floor(at / blocks.yz);
    let rank = fract(sin(dot(cell, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    let soft = blocks.w / max(blocks.y, blocks.z);
    return run_shown(rank, 1.0, blocks.x, soft);
}

// How much of the picture at `uv` the Transition effects leave shown.
fn clip_wipes_shown(uv: vec2<f32>) -> f32 {
    let size = picture_size();
    let at = uv * size;
    return linear_wipe_shown(at, size) * radial_wipe_shown(at, size) * blinds_shown(at)
        * blocks_shown(at);
}
