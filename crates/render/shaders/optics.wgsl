// Magnify, which enlarges part of the picture before anything else colors it, and
// Lens Flare, which adds light over the finished picture.

// Where the picture at `uv` is read from once Magnify enlarges it about its center
// within its circle or square. The feather and opacity ease the read back toward the
// unmagnified picture, like the rim of a glass, which keeps to one read of it.
fn magnified(uv: vec2<f32>) -> vec2<f32> {
    let lens = placement.optics_magnify;
    let detail = placement.optics_detail;
    if detail.z <= 0.0 || lens.w <= 0.0 {
        return uv;
    }
    let away = (uv - lens.xy) * picture_size();
    var reach = length(away);
    if detail.y > 0.5 {
        reach = max(abs(away.x), abs(away.y));
    }
    let cover = clamp((lens.w - reach) / max(detail.x, 1.0), 0.0, 1.0) * detail.z;
    return mix(uv, lens.xy + (uv - lens.xy) / lens.z, cover);
}

// A color for the `index`th reflection, cycling round the hues.
fn reflection_tint(index: f32) -> vec3<f32> {
    let hue = fract(index * 0.37 + 0.1);
    return 0.55 + 0.45 * cos(6.2831853 * (hue + vec3<f32>(0.0, 0.33, 0.67)));
}

// The light a bright source at the flare's center throws into the lens: a hot core
// in a wide glow, a faint halo, a streak for the zoom, and reflections of the
// aperture strung along the line through the middle of the picture. Sizes are in
// shares of the picture's longer side, so the flare keeps its look at any size.
fn flare_light(uv: vec2<f32>) -> vec3<f32> {
    let flare = placement.optics_flare;
    let picture = picture_size();
    let longer = max(picture.x, picture.y);
    let p = uv * picture / longer;
    let light = flare.xy * picture / longer;
    let middle = 0.5 * picture / longer;
    // Glow size, halo radius, streak strength, then the reflections' count, first
    // position, spacing and size, per lens.
    var glow = 0.12;
    var halo = 0.1;
    var streak = 1.0;
    var reflections = vec4<f32>(7.0, 0.5, 0.27, 0.025);
    if flare.w > 1.5 {
        glow = 0.07;
        halo = 0.06;
        streak = 0.3;
        reflections = vec4<f32>(5.0, 0.8, 0.12, 0.03);
    } else if flare.w > 0.5 {
        glow = 0.2;
        halo = 0.18;
        streak = 0.0;
        reflections = vec4<f32>(4.0, 0.7, 0.45, 0.07);
    }
    let offset = p - light;
    let reach = length(offset);
    var sum = vec3<f32>(1.0, 0.95, 0.85) * (exp(-reach / 0.02) + 0.35 * exp(-reach / glow));
    let ring = (reach - halo) / 0.012;
    sum += vec3<f32>(0.6, 0.8, 1.0) * 0.12 * exp(-ring * ring);
    sum += vec3<f32>(1.0, 0.9, 0.8) * streak * 0.4 * exp(-abs(offset.y) / 0.003)
        * exp(-abs(offset.x) / 0.3);
    let axis = middle - light;
    for (var index = 0.0; index < reflections.x; index += 1.0) {
        let at = light + axis * (reflections.y + index * reflections.z);
        let radius = reflections.w * (0.6 + fract(index * 0.618) * 0.8);
        let disc = 1.0 - smoothstep(radius * 0.8, radius, length(p - at));
        sum += reflection_tint(index) * 0.06 * disc;
    }
    return sum * flare.z;
}

fn flared(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let share = 1.0 - placement.optics_detail.w;
    if placement.optics_flare.z <= 0.0 || share <= 0.0 {
        return color;
    }
    let added = flare_light(uv) * share;
    return vec4<f32>(color.rgb + added, max(color.a, clamp(dot(added, LUMA), 0.0, 1.0)));
}
