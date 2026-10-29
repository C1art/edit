
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
