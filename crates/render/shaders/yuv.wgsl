// Turns a picture's planes into linear RGB, at the luma plane's size or shrunk. The
// planes, and `luma_at`, `cb_at` and `cr_at`, which read them as 0 to 1, come first.

struct Yuv {
    // Columns of the matrix from offset luma and chroma to gamma-encoded RGB.
    matrix: mat3x3<f32>,
    // Subtracted from the samples first, then what turns a stored sample into 0 to 1.
    offset: vec4<f32>,
    // Picks blue- and red-difference out of their texels.
    cb_mask: vec4<f32>,
    cr_mask: vec4<f32>,
    // Chroma texels per luma pixel, then how far right of that the chroma was sited.
    chroma: vec4<f32>,
    // Luma pixels across one output pixel, then the picture's size in them, which a
    // decoder's padded texture is larger than.
    footprint: vec4<f32>,
};

@group(0) @binding(3) var<uniform> yuv: Yuv;

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

fn chroma_texel(at: vec2<i32>) -> vec2<f32> {
    return vec2<f32>(dot(cb_at(at), yuv.cb_mask), dot(cr_at(at), yuv.cr_mask));
}

// Blue- and red-difference at `at`, in chroma texels, blended from the nearest four.
// Sixteen-bit planes can't be filtered by the sampler everywhere, so this does it.
fn chroma_at(at: vec2<f32>) -> vec2<f32> {
    let corner = at - 0.5;
    let base = floor(corner);
    let weight = corner - base;
    let last = vec2<i32>(ceil(yuv.footprint.zw * yuv.chroma.xy)) - 1;
    let low = clamp(vec2<i32>(base), vec2<i32>(0), last);
    let high = clamp(vec2<i32>(base) + 1, vec2<i32>(0), last);
    let top = mix(chroma_texel(low), chroma_texel(vec2<i32>(high.x, low.y)), weight.x);
    let bottom = mix(chroma_texel(vec2<i32>(low.x, high.y)), chroma_texel(high), weight.x);
    return mix(top, bottom, weight.y);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let footprint = yuv.footprint.xy;
    // Every luma pixel the output pixel covers counts the same, up to a limit only
    // thumbnails of big pictures reach. At full size the one tap reads its own pixel.
    let taps = clamp(round(footprint), vec2<f32>(1.0), vec2<f32>(8.0));
    let step = footprint / taps;
    let first = (position.xy - 0.5) * footprint + 0.5 * step;
    let last = vec2<i32>(yuv.footprint.zw) - 1;
    var y = 0.0;
    for (var row = 0.0; row < taps.y; row += 1.0) {
        for (var column = 0.0; column < taps.x; column += 1.0) {
            let at = vec2<i32>(first + vec2<f32>(column, row) * step);
            y += luma_at(min(at, last));
        }
    }
    y /= taps.x * taps.y;
    // Chroma is no finer than luma, so one sample from the middle of the output pixel
    // stands for it.
    let middle = position.xy * footprint;
    let chroma = chroma_at(middle * yuv.chroma.xy + vec2<f32>(yuv.chroma.z, 0.0));
    let rgb = yuv.matrix * (vec3<f32>(y, chroma) - yuv.offset.xyz);
    // Matches what sampling the same picture uploaded as sRGB-encoded RGBA8 gives.
    return vec4<f32>(srgb_to_linear(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
