// Lighting Effects: the picture as a surface under up to five lights, in linear light.

fn lamp_falloff(kind: f32, p: vec2<f32>, center: vec2<f32>, shape: vec4<f32>, focus: f32, picture: vec2<f32>) -> f32 {
    if kind < 1.5 {
        return 1.0;
    }
    let offset = p - center;
    if kind < 2.5 {
        return 1.0 - smoothstep(0.0, shape.x * picture.x, length(offset));
    }
    let turn = vec2<f32>(cos(shape.z), sin(shape.z));
    let along = vec2<f32>(dot(offset, turn), dot(offset, vec2<f32>(-turn.y, turn.x)));
    let reach = length(along / (shape.xy * picture.x));
    return 1.0 - smoothstep(focus, 1.0 + 1e-3, reach);
}

fn surface_normal(uv: vec2<f32>, picture: vec2<f32>, bump: f32, white_is_high: bool) -> vec3<f32> {
    if bump <= 0.0 {
        return vec3<f32>(0.0, 0.0, 1.0);
    }
    let step = 1.0 / picture;
    let height = surface_slope(uv, step, white_is_high);
    return normalize(vec3<f32>(-height * bump, 1.0));
}

// The surface's slope across and down at `uv`, from the picture's brightness.
fn surface_slope(uv: vec2<f32>, step: vec2<f32>, white_is_high: bool) -> vec2<f32> {
    let level = vec4<f32>(
        dot(graded(picture_at(uv - vec2<f32>(step.x, 0.0))), LUMA),
        dot(graded(picture_at(uv + vec2<f32>(step.x, 0.0))), LUMA),
        dot(graded(picture_at(uv - vec2<f32>(0.0, step.y))), LUMA),
        dot(graded(picture_at(uv + vec2<f32>(0.0, step.y))), LUMA),
    );
    let sign = select(-1.0, 1.0, white_is_high);
    return vec2<f32>(level.y - level.x, level.w - level.z) * 0.5 * sign;
}

fn lighting_effects(rgb: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let flags = placement.lighting_flags;
    if flags.x < 0.5 {
        return rgb;
    }
    // #if lighting_flags.x
    let surface = placement.lighting_surface;
    let ambience = placement.lighting_ambience;
    let picture = picture_size();
    let p = uv * picture;
    let normal = surface_normal(uv, picture, surface.w, flags.y > 0.5);
    var diffuse = ambience.rgb * ambience.w;
    var highlight = vec3<f32>(0.0);
    let shininess = mix(2.0, 200.0, surface.x);
    for (var i = 0; i < 5; i++) {
        let head = placement.lighting_lamps[i * 3];
        if head.x < 0.5 {
            continue;
        }
        let tint = placement.lighting_lamps[i * 3 + 1];
        let shape = placement.lighting_lamps[i * 3 + 2];
        let center = head.yz * picture;
        let height = shape.x * picture.x;
        var toward = vec3<f32>(center - p, height * 0.5);
        if head.x < 1.5 {
            toward = vec3<f32>(center - picture * 0.5, height);
        }
        let light = normalize(toward);
        let strength = lamp_falloff(head.x, p, center, shape, tint.w, picture) * head.w;
        diffuse += tint.rgb * max(dot(normal, light), 0.0) * strength;
        let halfway = normalize(light + vec3<f32>(0.0, 0.0, 1.0));
        let shine = pow(max(dot(normal, halfway), 0.0), shininess) * surface.x * strength;
        highlight += tint.rgb * mix(vec3<f32>(1.0), rgb, surface.y) * shine;
    }
    return (rgb * diffuse + highlight) * surface.z;
    // #else
    return rgb;
    // #endif
}
