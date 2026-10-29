// Planes of one byte a sample, read as 0 to 1 by the texture.

@group(0) @binding(0) var luma: texture_2d<f32>;
@group(0) @binding(1) var cb_plane: texture_2d<f32>;
@group(0) @binding(2) var cr_plane: texture_2d<f32>;

fn luma_at(at: vec2<i32>) -> f32 {
    return textureLoad(luma, at, 0).r;
}

fn cb_at(at: vec2<i32>) -> vec4<f32> {
    return textureLoad(cb_plane, at, 0);
}

fn cr_at(at: vec2<i32>) -> vec4<f32> {
    return textureLoad(cr_plane, at, 0);
}
