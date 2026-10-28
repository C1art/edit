// Counts the presented frame's pixels into a scope's bins. Levels are measured on the
// display-encoded signal, as broadcast scopes read it.

struct Params {
    // Source width and height, columns in use, levels.
    size_columns_levels: vec4<u32>,
    // 0 luma waveform, 1 RGB parade, 2 vectorscope.
    scope: vec4<u32>,
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

fn level_of(value: f32, levels: u32) -> u32 {
    return min(u32(clamp(value, 0.0, 1.0) * f32(levels)), levels - 1u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = params.size_columns_levels.xy;
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let columns = params.size_columns_levels.z;
    let levels = params.size_columns_levels.w;
    let encoded = clamp(srgb_encode(textureLoad(source, vec2<u32>(id.xy), 0).rgb), vec3<f32>(0.0), vec3<f32>(1.0));
    let luma = dot(encoded, LUMA);
    switch params.scope.x {
        case 1u: {
            let third = columns / 3u;
            let column = id.x * third / size.x;
            for (var channel = 0u; channel < 3u; channel++) {
                let level = level_of(encoded[channel], levels);
                atomicAdd(&counts[level * columns + channel * third + column], 1u);
            }
        }
        case 2u: {
            // BT.709 colour difference, scaled to fill the scope at full saturation.
            let cb = (encoded.b - luma) / 1.8556 + 0.5;
            let cr = (encoded.r - luma) / 1.5748 + 0.5;
            atomicAdd(&counts[level_of(cr, levels) * levels + level_of(cb, levels)], 1u);
        }
        default: {
            let column = id.x * columns / size.x;
            atomicAdd(&counts[level_of(luma, levels) * columns + column], 1u);
        }
    }
}
