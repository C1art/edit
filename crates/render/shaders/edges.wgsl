// Effects on the picture's edges: Roughen Edges wears them into a fractal outline,
// Bevel Edges and Bevel Alpha light them as raised, Alpha Glow surrounds them with
// color, and Color Emboss lights the picture's relief in its own colors.

const ROUGHEN_TAPS: i32 = 16;
const GLOW_RINGS: i32 = 5;
const GLOW_SPOKES: i32 = 8;

fn light_toward(angle: f32) -> vec2<f32> {
    return vec2<f32>(sin(angle), -cos(angle));
}

// A face turned toward the light by `facing` (-1 to 1) brightens toward its color; one
// turned away darkens.
fn lit(rgb: vec3<f32>, facing: f32, light: vec3<f32>) -> vec3<f32> {
    if facing >= 0.0 {
        return mix(rgb, light, facing);
    }
    return rgb * (1.0 + facing);
}

// How much of the picture Roughen Edges keeps at `uv`, then how much of what is kept
// shows the edge color.
fn roughened(uv: vec2<f32>) -> vec2<f32> {
    let settings = placement.edges_roughen;
    if settings.x <= 0.0 {
        return vec2<f32>(1.0, 0.0);
    }
    let noise = placement.edges_roughen_noise;
    let picture = picture_size();
    // The covered share of a disc as wide as the border: a half on an edge, all of it
    // the border's width inside, so it reads as the depth into the picture.
    var covered = 0.0;
    for (var tap = 0; tap < ROUGHEN_TAPS; tap++) {
        let radius = sqrt((f32(tap) + 0.5) / f32(ROUGHEN_TAPS)) * settings.x;
        let angle = f32(tap) * GOLDEN_ANGLE;
        covered += cast_alpha(uv + vec2<f32>(cos(angle), sin(angle)) * radius / picture);
    }
    let depth = saturate(covered / f32(ROUGHEN_TAPS) * 2.0 - 1.0);
    let at = (uv * picture + noise.zw) / settings.z;
    let fractal = saturate(turbulence(vec3<f32>(at, noise.x), settings.w) + 0.5);
    // Kept short of the full depth, so the fractal never eats into the inside.
    let cut = mix(0.5, fractal, noise.y) * 0.75;
    let kept = smoothstep(cut - settings.y, cut + settings.y, depth);
    let band = cut + settings.y;
    let colored = 1.0 - smoothstep(band, band + 0.2, depth);
    return vec2<f32>(kept, colored * placement.edges_roughen_color.w);
}

fn roughen_colored(rgb: vec3<f32>, colored: f32) -> vec3<f32> {
    return mix(rgb, placement.edges_roughen_color.rgb, colored);
}

// Bevel Edges: the frame's border lit as faces sloping out to its nearest side.
fn bevel_edges(rgb: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let bevel = placement.edges_bevel;
    if bevel.x <= 0.0 || bevel.z <= 0.0 {
        return rgb;
    }
    let picture = picture_size();
    let inside = vec4<f32>(uv, vec2<f32>(1.0) - uv) * vec4<f32>(picture, picture);
    let nearest = min(min(inside.x, inside.y), min(inside.z, inside.w));
    if nearest >= bevel.x * min(picture.x, picture.y) {
        return rgb;
    }
    var outward = vec2<f32>(-1.0, 0.0);
    if nearest == inside.y {
        outward = vec2<f32>(0.0, -1.0);
    } else if nearest == inside.z {
        outward = vec2<f32>(1.0, 0.0);
    } else if nearest == inside.w {
        outward = vec2<f32>(0.0, 1.0);
    }
    let facing = dot(outward, light_toward(bevel.y)) * bevel.z;
    return lit(rgb, facing, placement.edges_bevel_color.rgb);
}

// Bevel Alpha: faces sloping down the picture's coverage, as wide as the thickness.
fn bevel_alpha(rgb: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let bevel = placement.edges_bevel_alpha;
    if bevel.x <= 0.0 || bevel.z <= 0.0 {
        return rgb;
    }
    let step = bevel.x / picture_size();
    let across = vec2<f32>(step.x, 0.0);
    let down = vec2<f32>(0.0, step.y);
    let slope = vec2<f32>(
        cast_alpha(uv + across) - cast_alpha(uv - across),
        cast_alpha(uv + down) - cast_alpha(uv - down),
    ) * 0.5;
    let steepness = length(slope);
    if steepness <= 1e-4 {
        return rgb;
    }
    // Coverage falls away outward, so the face looks down the slope.
    let outward = -slope / steepness;
    let facing = dot(outward, light_toward(bevel.y)) * bevel.z * saturate(steepness * 2.0);
    return lit(rgb, facing, placement.edges_bevel_alpha_color.rgb);
}

// Color Emboss: Emboss's relief lighting the picture's colors rather than grey.
fn color_emboss(rgb: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let emboss = placement.edges_emboss;
    if emboss.w <= 0.0 {
        return rgb;
    }
    let toward = light_toward(emboss.x) * emboss.y / picture_size();
    let rise = encoded_luma(uv - toward) - encoded_luma(uv + toward);
    let embossed = to_linear(to_gamma(rgb) + vec3<f32>(rise * emboss.z * 2.0));
    return mix(rgb, embossed, emboss.w);
}

fn edge_lit(rgb: vec3<f32>, pinned: vec2<f32>, uv: vec2<f32>) -> vec3<f32> {
    return bevel_alpha(bevel_edges(color_emboss(rgb, uv), pinned), pinned);
}

// Alpha Glow around the picture's coverage: its color, then how strongly it shows. The
// glow takes the strongest of the rings of picture around `uv`, each fainter the
// further out it lies when it fades out.
fn alpha_glow(uv: vec2<f32>) -> vec4<f32> {
    let glow = placement.edges_glow;
    if glow.x <= 0.0 || glow.y <= 0.0 {
        return vec4<f32>(0.0);
    }
    let picture = picture_size();
    var strongest = 0.0;
    var reach = 0.0;
    for (var ring = 1; ring <= GLOW_RINGS; ring++) {
        let fraction = f32(ring) / f32(GLOW_RINGS);
        // Alternate rings turn half a spoke, so the glow's outline stays round.
        let turn = 0.5 * f32(ring % 2);
        var most = 0.0;
        for (var spoke = 0; spoke < GLOW_SPOKES; spoke++) {
            let angle = (f32(spoke) + turn) * TAU / f32(GLOW_SPOKES);
            let toward = vec2<f32>(cos(angle), sin(angle)) * fraction * glow.x / picture;
            most = max(most, cast_alpha(uv + toward));
        }
        let fade = select(1.0, 1.0 - fraction + 1.0 / f32(GLOW_RINGS), glow.w > 0.5);
        if most * fade > strongest {
            strongest = most * fade;
            reach = fraction;
        }
    }
    let color = mix(placement.edges_glow_start.rgb, placement.edges_glow_end.rgb, reach);
    return vec4<f32>(color, saturate(strongest * glow.y));
}
