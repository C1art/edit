// Counts the presented frame's pixels by column and luma level. Levels are measured on
// the display-encoded signal, as a broadcast waveform reads it.

struct Params {
    // Source width and height, columns in use, levels.
    size_columns_levels: vec4<u32>,
};

const LUMA: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> counts: array<atomic<u32>>;
@group(0) @binding(2) var<uniform> params: Params;

// Loading from the sRGB output decodes to linear, so encode again.
fn srgb_encode(linear: vec3<f32>) -> vec3<f32> {
    let low = linear * 12.92;
    let high = 1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(high, low, linear <= vec3<f32>(0.0031308));
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = params.size_columns_levels.xy;
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let columns = params.size_columns_levels.z;
    let levels = params.size_columns_levels.w;
    let encoded = srgb_encode(textureLoad(source, vec2<u32>(id.xy), 0).rgb);
    let luma = clamp(dot(encoded, LUMA), 0.0, 1.0);
    let column = id.x * columns / size.x;
    let level = min(u32(luma * f32(levels)), levels - 1u);
    atomicAdd(&counts[level * columns + column], 1u);
}
