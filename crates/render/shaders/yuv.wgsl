// Turns a 4:2:0 picture's planes into linear RGB at the luma plane's size.

struct Yuv {
    // Columns of the matrix from offset luma and chroma to gamma-encoded RGB.
    matrix: mat3x3<f32>,
    // Subtracted from the sampled luma and chroma first.
    offset: vec4<f32>,
    // Picks blue- and red-difference out of their texels.
    cb_mask: vec4<f32>,
    cr_mask: vec4<f32>,
    // Added to the chroma UV so it samples where the chroma was sited, then the scale
    // from luma pixels to chroma UV.
    chroma_shift_scale: vec4<f32>,
};

@group(0) @binding(0) var luma: texture_2d<f32>;
@group(0) @binding(1) var cb_plane: texture_2d<f32>;
@group(0) @binding(2) var cr_plane: texture_2d<f32>;
@group(0) @binding(3) var chroma_sampler: sampler;
@group(0) @binding(4) var<uniform> yuv: Yuv;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    );
    return vec4<f32>(corners[index], 0.0, 1.0);
}

fn srgb_to_linear(encoded: vec3<f32>) -> vec3<f32> {
    let low = encoded / 12.92;
    let high = pow((encoded + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, encoded <= vec3<f32>(0.04045));
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let y = textureLoad(luma, vec2<i32>(position.xy), 0).r;
    let uv = position.xy * yuv.chroma_shift_scale.zw + yuv.chroma_shift_scale.xy;
    let cb = dot(textureSampleLevel(cb_plane, chroma_sampler, uv, 0.0), yuv.cb_mask);
    let cr = dot(textureSampleLevel(cr_plane, chroma_sampler, uv, 0.0), yuv.cr_mask);
    let rgb = yuv.matrix * (vec3<f32>(y, cb, cr) - yuv.offset.xyz);
    // Matches what sampling the same picture uploaded as sRGB-encoded RGBA8 gives.
    return vec4<f32>(srgb_to_linear(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
