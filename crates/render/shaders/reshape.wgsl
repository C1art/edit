// Mirror and Corner Pin: warps of the whole picture, measured in fractions of it.

// Reflects the side of the mirror's line that its normal points to from the other.
fn mirrored(uv: vec2<f32>) -> vec2<f32> {
    let mirror = placement.reshape_mirror;
    if mirror.w < 0.5 {
        return uv;
    }
    let picture = picture_size();
    let aspect = picture / max(picture.x, picture.y);
    let normal = vec2<f32>(cos(mirror.z), sin(mirror.z));
    let p = (uv - mirror.xy) * aspect;
    let beyond = dot(p, normal);
    if beyond <= 0.0 {
        return uv;
    }
    return (p - 2.0 * beyond * normal) / aspect + mirror.xy;
}

// Where the point at `uv` of the pinned picture lies in the picture before pinning;
// outside it where the plane would face away.
fn unpinned(uv: vec2<f32>) -> vec2<f32> {
    let rows = placement.reshape_unpin;
    if rows[0].w < 0.5 {
        return uv;
    }
    let point = vec3<f32>(uv, 1.0);
    let w = dot(rows[2].xyz, point);
    if w <= 1e-6 {
        return vec2<f32>(-1.0);
    }
    return vec2<f32>(dot(rows[0].xyz, point), dot(rows[1].xyz, point)) / w;
}
