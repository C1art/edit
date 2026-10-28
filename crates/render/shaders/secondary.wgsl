// Lumetri's HSL Secondary: a key on hue, saturation and lightness, and the keyed
// colors' own correction.

// Hue in turns, saturation and lightness of gamma-encoded color, as a color picker
// shows them.
fn hsl(rgb: vec3<f32>) -> vec3<f32> {
    let c = clamp(to_gamma(rgb), vec3<f32>(0.0), vec3<f32>(1.0));
    let high = max(max(c.r, c.g), c.b);
    let low = min(min(c.r, c.g), c.b);
    let lightness = (high + low) * 0.5;
    let spread = high - low;
    if spread < 1e-5 {
        return vec3<f32>(0.0, 0.0, lightness);
    }
    let saturation = spread / max(1.0 - abs(2.0 * lightness - 1.0), 1e-5);
    var hue: f32;
    if high == c.r {
        hue = (c.g - c.b) / spread;
    } else if high == c.g {
        hue = (c.b - c.r) / spread + 2.0;
    } else {
        hue = (c.r - c.g) / spread + 4.0;
    }
    return vec3<f32>(fract(hue / 6.0), min(saturation, 1.0), lightness);
}

// 1 inside low..high, easing to 0 over `soft` beyond either end.
fn within(value: f32, low: f32, high: f32, soft: f32) -> f32 {
    return smoothstep(low - soft, low, value) * (1.0 - smoothstep(high, high + soft, value));
}

fn secondary_key(rgb: vec3<f32>) -> f32 {
    let key = placement.secondary_key;
    let ranges = placement.secondary_ranges;
    let soft = max(key.z, 1e-3);
    let color = hsl(rgb);
    let hue_distance = abs(fract(color.x - key.x + 0.5) - 0.5);
    // Grey has no hue, so the saturation range alone decides whether it is keyed.
    let hue = 1.0 - smoothstep(key.y, key.y + soft * 0.5, hue_distance);
    let matte = hue * within(color.y, ranges.x, ranges.y, soft) * within(color.z, ranges.z, ranges.w, soft);
    return mix(matte, 1.0 - matte, placement.secondary_wheel.w);
}

// The keyed colors' own white balance, contrast, wheel and saturation, blended in by
// the key.
fn secondary(rgb_in: vec3<f32>) -> vec3<f32> {
    let mode = placement.secondary_key.w;
    if mode < 0.5 {
        return rgb_in;
    }
    let keyed = secondary_key(rgb_in);
    if mode > 1.5 {
        return mix(vec3<f32>(MID_GREY), rgb_in, keyed);
    }
    let balance = placement.secondary_balance;
    let w = placement.secondary_wheel;
    let white = vec3<f32>(1.0 + 0.3 * balance.x, 1.0 - 0.3 * balance.y, 1.0 - 0.3 * balance.x);
    let hue = vec3<f32>(1.5748 * w.y, -0.1873 * w.x - 0.4681 * w.y, 1.8556 * w.x);
    var rgb = rgb_in * white * exp2(hue * WHEEL_STOPS + vec3<f32>(w.z * LEVEL_STOPS));
    let pivot = vec3<f32>(MID_GREY);
    rgb = pivot * pow(max(rgb, vec3<f32>(0.0)) / pivot, vec3<f32>(1.0 + balance.z));
    rgb = saturate_color(rgb, balance.w, 0.0);
    return mix(rgb_in, rgb, keyed);
}
