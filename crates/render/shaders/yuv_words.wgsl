// Planes of one 16-bit word a sample, holding as many bits as the source has.

@group(0) @binding(0) var luma: texture_2d<u32>;
@group(0) @binding(1) var cb_plane: texture_2d<u32>;
@group(0) @binding(2) var cr_plane: texture_2d<u32>;

fn luma_at(at: vec2<i32>) -> f32 {
    return f32(textureLoad(luma, at, 0).r) * yuv.offset.w;
}

fn cb_at(at: vec2<i32>) -> vec4<f32> {
    return vec4<f32>(textureLoad(cb_plane, at, 0)) * yuv.offset.w;
}

fn cr_at(at: vec2<i32>) -> vec4<f32> {
    return vec4<f32>(textureLoad(cr_plane, at, 0)) * yuv.offset.w;
}
