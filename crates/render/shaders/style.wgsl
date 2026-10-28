
// The picture style's effects: warps that move where the picture is read from, a
// directional blur and offset when reading it, then outlines, tone and grain on the
// encoded result, and a shadow cast behind it.

const BLUR_TAPS: i32 = 24;
const SHADOW_TAPS: i32 = 32;
const TAU: f32 = 6.2831853;
// Turning each tap by this spreads them evenly over a disc.
const GOLDEN_ANGLE: f32 = 2.3999632;

fn picture_size() -> vec2<f32> {
    return max(placement.mask_space.xy, vec2<f32>(1.0));
}

// Pushes the picture back and forth across waves that run along their direction.
fn wave(uv: vec2<f32>) -> vec2<f32> {
    let wave = placement.style_wave;
    if wave.x == 0.0 {
        return uv;
    }
    let picture = picture_size();
    let along = vec2<f32>(sin(wave.z), -cos(wave.z));
    let across = vec2<f32>(-along.y, along.x);
    let waves = dot(uv * picture, along) / wave.y - wave.w;
    return uv + across * wave.x * sin(TAU * waves) / picture;
}

fn lattice(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z) * 2.0 - 1.0;
}

// Smooth noise from -1 to 1, blending the random values at the lattice's corners.
fn value_noise(p: vec3<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(lattice(cell), lattice(cell + vec3<f32>(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(
        lattice(cell + vec3<f32>(0.0, 1.0, 0.0)),
        lattice(cell + vec3<f32>(1.0, 1.0, 0.0)),
        u.x,
    );
    let x01 = mix(
        lattice(cell + vec3<f32>(0.0, 0.0, 1.0)),
        lattice(cell + vec3<f32>(1.0, 0.0, 1.0)),
        u.x,
    );
    let x11 = mix(
        lattice(cell + vec3<f32>(0.0, 1.0, 1.0)),
        lattice(cell + vec3<f32>(1.0, 1.0, 1.0)),
        u.x,
    );
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

// Noise with a layer of half the size and half the strength for each step of
// complexity, the last layer fading in with its fraction.
fn turbulence(p: vec3<f32>, layers: f32) -> f32 {
    var sum = 0.0;
    var total = 0.0;
    var strength = 1.0;
    var scale = 1.0;
    for (var layer = 0; layer < 10; layer++) {
        let share = clamp(layers - f32(layer), 0.0, 1.0);
        if share <= 0.0 {
            break;
        }
        sum += value_noise(vec3<f32>(p.xy * scale, p.z)) * strength * share;
        total += strength * share;
        strength *= 0.5;
        scale *= 2.0;
    }
    return sum / max(total, 1e-4);
}

// Pushes the picture about along two unrelated turbulent noises.
fn turbulent(uv: vec2<f32>) -> vec2<f32> {
    let turbulence_settings = placement.style_turbulence;
    let center = placement.style_turbulence_center;
    if center.w < 0.5 {
        return uv;
    }
    let picture = picture_size();
    let at = (uv - center.xy) * picture / turbulence_settings.y;
    // Evolution moves through the noise's third dimension.
    let p = vec3<f32>(at, turbulence_settings.w * 4.0);
    let push = vec2<f32>(
        turbulence(p, turbulence_settings.z),
        turbulence(p + vec3<f32>(71.3, 19.7, 37.1), turbulence_settings.z),
    );
    return uv + push * turbulence_settings.x / picture;
}

// Where the picture at `uv` is read from once waved, lensed, twirled and cut into
// mosaic blocks. Warps measure in square units, so circles stay round on any picture.
fn warp(uv_in: vec2<f32>) -> vec2<f32> {
    let picture = picture_size();
    let aspect = picture / max(picture.x, picture.y);
    let half_diagonal = length(aspect * 0.5);
    var p = (wave(turbulent(mirrored(uv_in))) - vec2<f32>(0.5)) * aspect;
    // The picture as seen on a ball: points at the rim show as they are, nearer the
    // middle they read from nearer it still, as an arc's length to its height.
    let sphere = placement.style_sphere;
    let sphere_center = (sphere.xy - vec2<f32>(0.5)) * aspect;
    let sphere_radius = sphere.z / max(picture.x, picture.y);
    let from_center = p - sphere_center;
    let reach = length(from_center);
    if sphere.w > 0.5 && reach < sphere_radius && reach > 0.0 {
        let read = sphere_radius * asin(reach / sphere_radius) * 2.0 / 3.14159265;
        p = sphere_center + from_center * read / reach;
    }
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
    return steadied(uv, aspect);
}

// Steadying undoes the camera's shake before any other effect sees the picture, so
// it maps nearest the source.
fn steadied(uv: vec2<f32>, aspect: vec2<f32>) -> vec2<f32> {
    let stabilize = placement.style_stabilize;
    if all(stabilize == vec4<f32>(0.0, 0.0, 0.0, 1.0)) {
        return uv;
    }
    let p = (uv - vec2<f32>(0.5)) * aspect;
    let turn = vec2<f32>(sin(stabilize.z), cos(stabilize.z));
    let turned = vec2<f32>(turn.y * p.x - turn.x * p.y, turn.x * p.x + turn.y * p.y);
    return (turned * stabilize.w + stabilize.xy) / aspect + vec2<f32>(0.5);
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

fn encoded_luma(uv: vec2<f32>) -> f32 {
    let encoded = to_gamma(max(input_grade(picture_at(uv).rgb), vec3<f32>(0.0)));
    return dot(encoded, vec3<f32>(0.2126, 0.7152, 0.0722));
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
    let emboss = placement.style_emboss;
    let untouched = all(tone == vec4<f32>(0.0, 0.0, 1.0, 0.0)) && grain.x <= 0.0
        && grain.z <= 0.0 && emboss.w <= 0.0;
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
    if emboss.w > 0.0 {
        // Lit from the light's side: a rise facing it is bright, one facing away dark.
        let toward = vec2<f32>(sin(emboss.x), -cos(emboss.x)) * emboss.y / picture_size();
        let rise = encoded_luma(uv - toward) - encoded_luma(uv + toward);
        let relief = clamp(0.5 + rise * emboss.z * 2.0, 0.0, 1.0);
        encoded = mix(encoded, vec3<f32>(relief), emboss.w);
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

// How much shadow the picture casts on `uv`, softened by averaging it over a disc.
fn shadow_alpha(uv: vec2<f32>) -> f32 {
    let shadow = placement.style_shadow;
    if shadow.w <= 0.0 {
        return 0.0;
    }
    let picture = picture_size();
    let caster = uv - shadow.xy / picture;
    if shadow.z < 0.5 {
        return cast_alpha(caster) * shadow.w;
    }
    var sum = 0.0;
    for (var tap = 0; tap < SHADOW_TAPS; tap++) {
        let radius = sqrt((f32(tap) + 0.5) / f32(SHADOW_TAPS)) * shadow.z;
        let angle = f32(tap) * GOLDEN_ANGLE;
        sum += cast_alpha(caster + vec2<f32>(cos(angle), sin(angle)) * radius / picture);
    }
    return sum / f32(SHADOW_TAPS) * shadow.w;
}
