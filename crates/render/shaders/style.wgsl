
// The picture style's effects: warps that move where the picture is read from, a
// directional blur and offset when reading it, then outlines, tone and grain on the
// encoded result.

const BLUR_TAPS: i32 = 24;

fn picture_size() -> vec2<f32> {
    return max(placement.mask_space.xy, vec2<f32>(1.0));
}

// Where the picture at `uv` is read from once lensed, twirled and cut into mosaic
// blocks. Warps measure in square units, so circles stay round on any picture.
fn warp(uv_in: vec2<f32>) -> vec2<f32> {
    let picture = picture_size();
    let aspect = picture / max(picture.x, picture.y);
    let half_diagonal = length(aspect * 0.5);
    var p = (uv_in - vec2<f32>(0.5)) * aspect;
    // Scaling with the squared radius, normalized so the corners stay put: a bulge
    // reads nearer the center there, magnifying it.
    let lens = placement.style_warp.w * 0.5;
    if lens != 0.0 {
        let r2 = dot(p, p) / (half_diagonal * half_diagonal);
        p *= (1.0 + lens * r2) / (1.0 + lens);
    }
    let shape = placement.style_shape;
    let radius = shape.w * half_diagonal;
    let distance = length(p);
    if shape.z != 0.0 && distance < radius {
        let falloff = 1.0 - distance / radius;
        let angle = -shape.z * falloff * falloff;
        let turn = vec2<f32>(sin(angle), cos(angle));
        p = vec2<f32>(turn.y * p.x - turn.x * p.y, turn.x * p.x + turn.y * p.y);
    }
    var uv = p / aspect + vec2<f32>(0.5);
    let blocks = shape.xy;
    if blocks.x >= 1.0 {
        uv.x = (floor(uv.x * blocks.x) + 0.5) / blocks.x;
    }
    if blocks.y >= 1.0 {
        uv.y = (floor(uv.y * blocks.y) + 0.5) / blocks.y;
    }
    return uv;
}

fn smeared(uv: vec2<f32>) -> vec4<f32> {
    let blur = placement.style_blur;
    if blur.x <= 0.0 {
        return textureSampleLevel(source, source_sampler, uv, 0.0);
    }
    let reach = vec2<f32>(sin(blur.y), -cos(blur.y)) * blur.x / picture_size();
    var sum = vec4<f32>(0.0);
    for (var tap = 0; tap < BLUR_TAPS; tap++) {
        let along = f32(tap) / f32(BLUR_TAPS - 1) * 2.0 - 1.0;
        sum += textureSampleLevel(source, source_sampler, uv + reach * along, 0.0);
    }
    return sum / f32(BLUR_TAPS);
}

// The picture at the warped `uv`, smeared and offset; transparent past its edges.
fn picture_at(uv: vec2<f32>) -> vec4<f32> {
    let offset = placement.style_warp;
    var color = smeared(uv);
    if offset.z > 0.0 {
        color = mix(color, smeared(fract(uv - offset.xy)), offset.z);
    }
    let inside = all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
    return color * f32(inside);
}

fn grain_hash(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(0.1031, 0.1030));
    q += dot(q, q.yx + 33.33);
    return fract((q.x + q.y) * q.x);
}

// Outlines from the encoded neighbors' differences, then brightness, contrast, gamma,
// posterizing and grain, all on encoded values as Premiere's effects work.
fn restyle(rgb_in: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let tone = placement.style_tone;
    let grain = placement.style_grain;
    let untouched = all(tone == vec4<f32>(0.0, 0.0, 1.0, 0.0)) && grain.x <= 0.0
        && grain.z <= 0.0;
    if untouched {
        return rgb_in;
    }
    var encoded = to_gamma(max(rgb_in, vec3<f32>(0.0)));
    if grain.z > 0.0 {
        let texel = 1.0 / vec2<f32>(textureDimensions(source));
        let across = to_gamma(input_grade(picture_at(uv + vec2<f32>(texel.x, 0.0)).rgb))
            - to_gamma(input_grade(picture_at(uv - vec2<f32>(texel.x, 0.0)).rgb));
        let down = to_gamma(input_grade(picture_at(uv + vec2<f32>(0.0, texel.y)).rgb))
            - to_gamma(input_grade(picture_at(uv - vec2<f32>(0.0, texel.y)).rgb));
        var edges = clamp((abs(across) + abs(down)) * 2.0, vec3<f32>(0.0), vec3<f32>(1.0));
        if grain.w < 0.5 {
            edges = vec3<f32>(1.0) - edges;
        }
        encoded = mix(encoded, edges, grain.z);
    }
    // Lowering contrast flattens toward grey; raising it steepens up to fourfold.
    let slope = select(1.0 + tone.y, 1.0 + 3.0 * tone.y, tone.y > 0.0);
    encoded = (encoded - vec3<f32>(0.5)) * slope + vec3<f32>(0.5) + tone.x * 0.4;
    encoded = pow(max(encoded, vec3<f32>(0.0)), vec3<f32>(tone.z));
    if tone.w >= 2.0 {
        let steps = tone.w - 1.0;
        encoded = round(clamp(encoded, vec3<f32>(0.0), vec3<f32>(1.0)) * steps) / steps;
    }
    if grain.x > 0.0 {
        let pixel = floor(uv * vec2<f32>(textureDimensions(source))) + grain.y;
        let noise = vec3<f32>(
            grain_hash(pixel),
            grain_hash(pixel + vec2<f32>(17.0, 0.0)),
            grain_hash(pixel + vec2<f32>(0.0, 29.0)),
        ) - vec3<f32>(0.5);
        encoded += noise * grain.x;
    }
    return to_linear(max(encoded, vec3<f32>(0.0)));
}
