// Turns a 4:2:0 picture's planes into linear RGB, at the luma plane's size or shrunk.

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
    // Luma pixels across one output pixel, then the size of a luma pixel in UV.
    footprint: vec4<f32>,
};

@group(0) @binding(0) var luma: texture_2d<f32>;
@group(0) @binding(1) var cb_plane: texture_2d<f32>;
@group(0) @binding(2) var cr_plane: texture_2d<f32>;
@group(0) @binding(3) var plane_sampler: sampler;
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

// Luma, blue- and red-difference at `at`, in luma pixels.
fn sample_at(at: vec2<f32>) -> vec3<f32> {
    let y = textureSampleLevel(luma, plane_sampler, at * yuv.footprint.zw, 0.0).r;
    let uv = at * yuv.chroma_shift_scale.zw + yuv.chroma_shift_scale.xy;
    let cb = dot(textureSampleLevel(cb_plane, plane_sampler, uv, 0.0), yuv.cb_mask);
    let cr = dot(textureSampleLevel(cr_plane, plane_sampler, uv, 0.0), yuv.cr_mask);
    return vec3<f32>(y, cb, cr);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let footprint = yuv.footprint.xy;
    // A tap on the corner between luma pixels averages four of them, so half as many
    // taps a side as the footprint is wide box-filter it. At full size the one tap
    // lands on the pixel's center and reads it unfiltered.
    let taps = max(round(footprint * 0.5), vec2<f32>(1.0));
    let step = footprint / taps;
    let first = (position.xy - 0.5) * footprint + 0.5 * step;
    var sum = vec3<f32>(0.0);
    for (var row = 0.0; row < taps.y; row += 1.0) {
        for (var column = 0.0; column < taps.x; column += 1.0) {
            sum += sample_at(first + vec2<f32>(column, row) * step);
        }
    }
    let rgb = yuv.matrix * (sum / (taps.x * taps.y) - yuv.offset.xyz);
    // Matches what sampling the same picture uploaded as sRGB-encoded RGBA8 gives.
    return vec4<f32>(srgb_to_linear(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
