// One direction of a separable Gaussian blur, from a whole texture to a whole target
// of the same size. Edges repeat through the clamping sampler.

struct Blur {
    // UV distance between taps, then the tap count each side.
    step_taps: vec4<f32>,
    // Each channel's sigma, in taps.
    sigma: vec4<f32>,
    // The rest of the shared uniform buffer.
    reserved: array<vec4<f32>, 3>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> blur: Blur;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[index];
    var out: VertexOutput;
    out.position = vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
    out.uv = corner;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let step = blur.step_taps.xy;
    let taps = i32(blur.step_taps.w);
    // An unblurred channel keeps only its centre tap.
    let sigma = max(blur.sigma, vec4<f32>(0.01));
    var sum = vec4<f32>(0.0);
    var weights = vec4<f32>(0.0);
    for (var i = -taps; i <= taps; i++) {
        let distance = vec4<f32>(f32(i)) / sigma;
        let weight = exp(-0.5 * distance * distance);
        sum += textureSampleLevel(source, source_sampler, in.uv + step * f32(i), 0.0) * weight;
        weights += weight;
    }
    return sum / weights;
}
