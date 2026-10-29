
// Track mattes are drawn over their fill with a blend state that keeps as much of
// the fill as the alpha they output, or as the rest of it when reversed.

@fragment
fn fs_matte_alpha(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, layer_color(in).a);
}

@fragment
fn fs_matte_luma(in: VertexOutput) -> @location(0) vec4<f32> {
    let matte = layer_color(in);
    // How bright the matte looks over black.
    return vec4<f32>(0.0, 0.0, 0.0, dot(srgb_encode(matte.rgb), LUMA) * matte.a);
}

// Set Matte's channels, each as encoded and scaled by the matte's own alpha.
@fragment
fn fs_matte_red(in: VertexOutput) -> @location(0) vec4<f32> {
    let matte = layer_color(in);
    return vec4<f32>(0.0, 0.0, 0.0, srgb_encode(matte.rgb).r * matte.a);
}

@fragment
fn fs_matte_green(in: VertexOutput) -> @location(0) vec4<f32> {
    let matte = layer_color(in);
    return vec4<f32>(0.0, 0.0, 0.0, srgb_encode(matte.rgb).g * matte.a);
}

@fragment
fn fs_matte_blue(in: VertexOutput) -> @location(0) vec4<f32> {
    let matte = layer_color(in);
    return vec4<f32>(0.0, 0.0, 0.0, srgb_encode(matte.rgb).b * matte.a);
}

@fragment
fn fs_matte_saturation(in: VertexOutput) -> @location(0) vec4<f32> {
    let matte = layer_color(in);
    let rgb = srgb_encode(matte.rgb);
    let high = max(rgb.r, max(rgb.g, rgb.b));
    let low = min(rgb.r, min(rgb.g, rgb.b));
    return vec4<f32>(0.0, 0.0, 0.0, select(0.0, (high - low) / high, high > 0.0) * matte.a);
}

@fragment
fn fs_matte_lightness(in: VertexOutput) -> @location(0) vec4<f32> {
    let matte = layer_color(in);
    let rgb = srgb_encode(matte.rgb);
    let lightness = (max(rgb.r, max(rgb.g, rgb.b)) + min(rgb.r, min(rgb.g, rgb.b))) / 2.0;
    return vec4<f32>(0.0, 0.0, 0.0, lightness * matte.a);
}
