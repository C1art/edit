// Difference Matte, Compound Blur and Texturize: the layer's group is the source and
// the other track's group is bound as the backdrop. Both are canvas-sized and hold
// colors multiplied by their coverage.

const COMBINE_TAPS: i32 = 16;

fn combine_load(texture: texture_2d<f32>, pixel: vec2<i32>) -> vec4<f32> {
    let last = vec2<i32>(textureDimensions(texture)) - 1;
    return textureLoad(texture, clamp(pixel, vec2<i32>(0), last), 0);
}

// The average of `texture` over a disc of `radius` pixels.
fn combine_disc(texture: texture_2d<f32>, center: vec2<f32>, radius: f32) -> vec4<f32> {
    if radius < 0.5 {
        return combine_load(texture, vec2<i32>(center));
    }
    var sum = vec4<f32>(0.0);
    for (var tap = 0; tap < COMBINE_TAPS; tap++) {
        let reach = sqrt((f32(tap) + 0.5) / f32(COMBINE_TAPS)) * radius;
        let angle = f32(tap) * GOLDEN_ANGLE;
        sum += combine_load(texture, vec2<i32>(center + vec2<f32>(cos(angle), sin(angle)) * reach));
    }
    return sum / f32(COMBINE_TAPS);
}

fn shown_color(premultiplied: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(premultiplied.rgb / max(premultiplied.a, 1e-6), premultiplied.a);
}

// How bright `premultiplied` looks over black, as displayed.
fn combine_luma(premultiplied: vec4<f32>) -> f32 {
    return dot(srgb_encode(shown_color(premultiplied).rgb), LUMA) * premultiplied.a;
}

fn difference_matte(center: vec2<f32>) -> vec4<f32> {
    let look = placement.combine;
    let layer = shown_color(combine_load(source, vec2<i32>(center)));
    let compared = shown_color(combine_disc(source, center, look.w));
    let other = shown_color(combine_disc(backdrop, center, look.w));
    let apart = abs(srgb_encode(compared.rgb) - srgb_encode(other.rgb));
    // Where the other picture is empty there is nothing to match.
    let difference = select(max(apart.r, max(apart.g, apart.b)), 1.0, other.a < 0.5);
    let kept = saturate((difference - look.y) / max(look.z, 1e-3));
    let view = placement.combine_view.x;
    if view > 1.5 {
        return vec4<f32>(vec3<f32>(kept), layer.a);
    }
    if view > 0.5 {
        return layer;
    }
    return vec4<f32>(layer.rgb, layer.a * kept);
}

fn compound_blur(center: vec2<f32>) -> vec4<f32> {
    let look = placement.combine;
    var amount = combine_luma(combine_load(backdrop, vec2<i32>(center)));
    if look.z > 0.5 {
        amount = 1.0 - amount;
    }
    return shown_color(combine_disc(source, center, amount * look.y));
}

fn texturized(center: vec2<f32>) -> vec4<f32> {
    let look = placement.combine;
    let layer = shown_color(combine_load(source, vec2<i32>(center)));
    let pixel = vec2<i32>(center);
    let x = vec2<i32>(1, 0);
    let y = vec2<i32>(0, 1);
    let slope = vec2<f32>(
        combine_luma(combine_load(backdrop, pixel + x)) - combine_luma(combine_load(backdrop, pixel - x)),
        combine_luma(combine_load(backdrop, pixel + y)) - combine_luma(combine_load(backdrop, pixel - y)),
    );
    let toward_light = vec2<f32>(sin(look.y), -cos(look.y));
    // Slopes facing the light catch more of it.
    let lit = 1.0 - dot(slope, toward_light) * look.z * 2.0;
    return vec4<f32>(srgb_decode(saturate(srgb_encode(layer.rgb) * lit)), layer.a);
}

@fragment
fn fs_combine(in: VertexOutput) -> @location(0) vec4<f32> {
    let center = in.position.xy;
    let kind = placement.combine.x;
    if kind > 2.5 {
        return texturized(center);
    }
    if kind > 1.5 {
        return compound_blur(center);
    }
    return difference_matte(center);
}
