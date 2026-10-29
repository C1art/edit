// Appended to blit.wgsl. Blends a layer with what lies under it: the backdrop is a copy
// of the target taken just before the layer is drawn, read at the fragment's own pixel.
// Blending happens on sRGB-encoded values, where the modes' formulas were designed, so
// it clips light brighter than white.

@group(2) @binding(0) var backdrop: texture_2d<f32>;

// In timeline::BlendMode's order.
const DISSOLVE: u32 = 1u;
const DARKEN: u32 = 2u;
const MULTIPLY: u32 = 3u;
const COLOR_BURN: u32 = 4u;
const LINEAR_BURN: u32 = 5u;
const DARKER_COLOR: u32 = 6u;
const LIGHTEN: u32 = 7u;
const SCREEN: u32 = 8u;
const COLOR_DODGE: u32 = 9u;
const LINEAR_DODGE: u32 = 10u;
const LIGHTER_COLOR: u32 = 11u;
const OVERLAY: u32 = 12u;
const SOFT_LIGHT: u32 = 13u;
const HARD_LIGHT: u32 = 14u;
const VIVID_LIGHT: u32 = 15u;
const LINEAR_LIGHT: u32 = 16u;
const PIN_LIGHT: u32 = 17u;
const HARD_MIX: u32 = 18u;
const DIFFERENCE: u32 = 19u;
const EXCLUSION: u32 = 20u;
const SUBTRACT: u32 = 21u;
const DIVIDE: u32 = 22u;
const HUE: u32 = 23u;
const SATURATION: u32 = 24u;
const COLOR: u32 = 25u;
const LUMINOSITY: u32 = 26u;

const BLEND_LUMA: vec3<f32> = vec3<f32>(0.3, 0.59, 0.11);

fn screen(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    return b + s - b * s;
}

fn color_burn(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let burned = 1.0 - min(vec3<f32>(1.0), (1.0 - b) / max(s, vec3<f32>(1e-6)));
    return select(select(burned, vec3<f32>(0.0), s <= vec3<f32>(0.0)), vec3<f32>(1.0), b >= vec3<f32>(1.0));
}

fn color_dodge(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let dodged = min(vec3<f32>(1.0), b / max(1.0 - s, vec3<f32>(1e-6)));
    return select(select(dodged, vec3<f32>(1.0), s >= vec3<f32>(1.0)), vec3<f32>(0.0), b <= vec3<f32>(0.0));
}

fn hard_light(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    return select(screen(b, 2.0 * s - 1.0), b * 2.0 * s, s <= vec3<f32>(0.5));
}

fn soft_light(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    let d = select(sqrt(b), ((16.0 * b - 12.0) * b + 4.0) * b, b <= vec3<f32>(0.25));
    let darker = b - (1.0 - 2.0 * s) * b * (1.0 - b);
    let lighter = b + (2.0 * s - 1.0) * (d - b);
    return select(lighter, darker, s <= vec3<f32>(0.5));
}

fn vivid_light(b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    return select(color_dodge(b, 2.0 * s - 1.0), color_burn(b, 2.0 * s), s <= vec3<f32>(0.5));
}

fn blend_luma(c: vec3<f32>) -> f32 {
    return dot(c, BLEND_LUMA);
}

fn clip_color(c: vec3<f32>) -> vec3<f32> {
    let l = blend_luma(c);
    let low = min(c.r, min(c.g, c.b));
    let high = max(c.r, max(c.g, c.b));
    var clipped = c;
    if low < 0.0 {
        clipped = l + (clipped - l) * l / max(l - low, 1e-6);
    }
    if high > 1.0 {
        clipped = l + (clipped - l) * (1.0 - l) / max(high - l, 1e-6);
    }
    return clipped;
}

fn set_luma(c: vec3<f32>, l: f32) -> vec3<f32> {
    return clip_color(c + (l - blend_luma(c)));
}

fn saturation_of(c: vec3<f32>) -> f32 {
    return max(c.r, max(c.g, c.b)) - min(c.r, min(c.g, c.b));
}

fn set_saturation(c: vec3<f32>, saturation: f32) -> vec3<f32> {
    let low = min(c.r, min(c.g, c.b));
    let range = saturation_of(c);
    if range <= 0.0 {
        return vec3<f32>(0.0);
    }
    return (c - low) * saturation / range;
}

fn blend(mode: u32, b: vec3<f32>, s: vec3<f32>) -> vec3<f32> {
    switch mode {
        case DARKEN: { return min(b, s); }
        case MULTIPLY: { return b * s; }
        case COLOR_BURN: { return color_burn(b, s); }
        case LINEAR_BURN: { return max(b + s - 1.0, vec3<f32>(0.0)); }
        case DARKER_COLOR: { return select(b, s, blend_luma(s) < blend_luma(b)); }
        case LIGHTEN: { return max(b, s); }
        case SCREEN: { return screen(b, s); }
        case COLOR_DODGE: { return color_dodge(b, s); }
        case LINEAR_DODGE: { return b + s; }
        case LIGHTER_COLOR: { return select(b, s, blend_luma(s) > blend_luma(b)); }
        case OVERLAY: { return hard_light(s, b); }
        case SOFT_LIGHT: { return soft_light(b, s); }
        case HARD_LIGHT: { return hard_light(b, s); }
        case VIVID_LIGHT: { return vivid_light(b, s); }
        case LINEAR_LIGHT: { return max(b + 2.0 * s - 1.0, vec3<f32>(0.0)); }
        case PIN_LIGHT: {
            return select(max(b, 2.0 * s - 1.0), min(b, 2.0 * s), s <= vec3<f32>(0.5));
        }
        case HARD_MIX: { return select(vec3<f32>(0.0), vec3<f32>(1.0), b + s >= vec3<f32>(1.0)); }
        case DIFFERENCE: { return abs(b - s); }
        case EXCLUSION: { return b + s - 2.0 * b * s; }
        case SUBTRACT: { return max(b - s, vec3<f32>(0.0)); }
        case DIVIDE: { return select(b / max(s, vec3<f32>(1e-6)), vec3<f32>(1.0), s <= vec3<f32>(0.0)); }
        case HUE: { return set_luma(set_saturation(s, saturation_of(b)), blend_luma(b)); }
        case SATURATION: { return set_luma(set_saturation(b, saturation_of(s)), blend_luma(b)); }
        case COLOR: { return set_luma(s, blend_luma(b)); }
        case LUMINOSITY: { return set_luma(b, blend_luma(s)); }
        default: { return s; }
    }
}

// A fixed pseudo-random value from 0 to 1 per pixel.
fn pixel_noise(pixel: vec2<f32>) -> f32 {
    return fract(sin(dot(pixel, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@fragment
fn fs_blend(in: VertexOutput) -> @location(0) vec4<f32> {
    let layer = layer_color(in);
    let mode = u32(placement.finish.w + 0.5);
    if mode == DISSOLVE {
        return vec4<f32>(layer.rgb, f32(pixel_noise(in.position.xy) < layer.a));
    }
    let under = textureLoad(backdrop, vec2<i32>(in.position.xy), 0);
    // Group canvases hold colors multiplied by their coverage.
    let b = srgb_encode(under.rgb / max(under.a, 1e-6));
    let s = srgb_encode(layer.rgb);
    let blended = srgb_decode(max(blend(mode, b, s), vec3<f32>(0.0)));
    // Where nothing lies underneath, the layer shows as itself.
    return vec4<f32>(mix(layer.rgb, blended, under.a), layer.a);
}
