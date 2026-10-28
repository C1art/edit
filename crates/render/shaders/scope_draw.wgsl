// Paints a scope's binned counts as a glowing trace.

struct Params {
    // Source width and height, columns in use, levels.
    size_columns_levels: vec4<u32>,
    // 0 luma waveform, 1 RGB parade, 2 vectorscope.
    scope: vec4<u32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// How quickly a bin saturates, relative to a column spread evenly over all levels.
const GLOW: f32 = 0.35;
const TRACE: vec3<f32> = vec3<f32>(0.45, 1.0, 0.55);
const PARADE: array<vec3<f32>, 3> = array<vec3<f32>, 3>(
    vec3<f32>(1.0, 0.3, 0.3), vec3<f32>(0.3, 1.0, 0.4), vec3<f32>(0.35, 0.5, 1.0),
);

@group(0) @binding(0) var<storage, read> counts: array<u32>;
@group(0) @binding(1) var<uniform> params: Params;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let uv = corners[index];
    var out: VertexOutput;
    out.position = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    out.uv = uv;
    return out;
}

fn glow(count: u32, even_share: f32) -> f32 {
    return 1.0 - exp(-GLOW * f32(count) / even_share);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let size = params.size_columns_levels.xy;
    let columns = params.size_columns_levels.z;
    let levels = params.size_columns_levels.w;
    let pixels = f32(size.x * size.y);
    let level = min(u32((1.0 - in.uv.y) * f32(levels)), levels - 1u);
    switch params.scope.x {
        case 1u: {
            let third = columns / 3u;
            let channel = min(u32(in.uv.x * 3.0), 2u);
            let column = min(u32(fract(in.uv.x * 3.0) * f32(third)), third - 1u);
            let count = counts[level * columns + channel * third + column];
            let even_share = pixels / f32(third) / f32(levels);
            return vec4<f32>(PARADE[channel] * glow(count, even_share), 1.0);
        }
        case 2u: {
            let cb = min(u32(in.uv.x * f32(levels)), levels - 1u);
            let count = counts[level * levels + cb];
            // Most pictures crowd near neutral, so faint colours need more gain.
            let even_share = pixels / f32(levels * levels) / 8.0;
            let chroma = vec2<f32>(in.uv.x, 1.0 - in.uv.y) - 0.5;
            // The hue a pixel binned here has, at mid luma.
            let hue = clamp(
                vec3<f32>(
                    0.5 + 1.5748 * chroma.y,
                    0.5 - 0.1873 * chroma.x - 0.4681 * chroma.y,
                    0.5 + 1.8556 * chroma.x,
                ),
                vec3<f32>(0.0),
                vec3<f32>(1.0),
            );
            return vec4<f32>(mix(vec3<f32>(1.0), hue, 0.6) * glow(count, even_share), 1.0);
        }
        default: {
            let column = min(u32(in.uv.x * f32(columns)), columns - 1u);
            let count = counts[level * columns + column];
            let even_share = pixels / f32(columns) / f32(levels);
            return vec4<f32>(TRACE * glow(count, even_share), 1.0);
        }
    }
}
