// Color Key, Leave Color and Gradient Ramp, which compare and blend encoded colors as
// Premiere's effects do.

const SQRT_3: f32 = 1.7320508;
const PI: f32 = 3.14159265;
// Below this chroma a color reads as grey, with no hue to match.
const GREY_CHROMA: f32 = 0.02;

// 0 at `encoded`, 1 across the color cube from it.
fn color_distance(rgb: vec3<f32>, encoded: vec3<f32>) -> f32 {
    return length(to_gamma(max(rgb, vec3<f32>(0.0))) - encoded) / SQRT_3;
}

// Hue in half turns from `encoded`'s, or 1 for greys.
fn hue_distance(rgb: vec3<f32>, encoded: vec3<f32>) -> f32 {
    let color = chroma(max(rgb, vec3<f32>(0.0)));
    let leave = chroma(to_linear(encoded));
    if length(color) < GREY_CHROMA || length(leave) < GREY_CHROMA {
        return 1.0;
    }
    return acos(clamp(dot(normalize(color), normalize(leave)), -1.0, 1.0)) / PI;
}

fn color_key_alpha(rgb: vec3<f32>) -> f32 {
    let key = placement.recolor_key;
    if key.w < 0.0 {
        return 1.0;
    }
    let softness = max(placement.recolor_amounts.w, 1e-4);
    return smoothstep(key.w, key.w + softness, color_distance(rgb, key.rgb));
}

fn leave_color(rgb: vec3<f32>) -> vec3<f32> {
    let leave = placement.recolor_leave;
    if leave.w < 0.0 {
        return rgb;
    }
    let amounts = placement.recolor_amounts;
    var distance = color_distance(rgb, leave.rgb);
    if amounts.z > 0.5 {
        distance = hue_distance(rgb, leave.rgb);
    }
    let kept = 1.0 - smoothstep(leave.w, leave.w + max(amounts.y, 1e-4), distance);
    return mix(rgb, vec3<f32>(dot(rgb, LUMA)), amounts.x * (1.0 - kept));
}

// Covers the picture at `uv` with the ramp, which fills it even where it was clear.
fn ramped(color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let end = placement.recolor_ramp_end;
    let inside = all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
    if end.w <= 0.0 || !inside {
        return color;
    }
    let points = placement.recolor_ramp_points;
    let start = placement.recolor_ramp_start;
    let picture = picture_size();
    let aspect = picture / max(picture.x, picture.y);
    let along = (points.zw - points.xy) * aspect;
    let p = (uv - points.xy) * aspect;
    var t = dot(p, along) / max(dot(along, along), 1e-8);
    if start.w > 0.5 {
        t = length(p) / max(length(along), 1e-4);
    }
    let fill = srgb_decode(mix(start.rgb, end.rgb, clamp(t, 0.0, 1.0)));
    return vec4<f32>(mix(color.rgb, fill, end.w), mix(color.a, 1.0, end.w));
}
