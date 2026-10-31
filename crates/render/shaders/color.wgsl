// Source color management: pictures in another color space, decoded by the texture as
// if sRGB, are brought into linear Rec. 709 with SDR white at 1.

// SMPTE ST 2084 constants.
const PQ_M1: f32 = 0.1593017578125;
const PQ_M2: f32 = 78.84375;
const PQ_C1: f32 = 0.8359375;
const PQ_C2: f32 = 18.8515625;
const PQ_C3: f32 = 18.6875;
const PQ_PEAK_NITS: f32 = 10000.0;
// ARIB STD-B67 constants, and the reference display Rec. 2100 assumes for HLG.
const HLG_A: f32 = 0.17883277;
const HLG_B: f32 = 0.28466892;
const HLG_C: f32 = 0.55991073;
const HLG_PEAK_NITS: f32 = 1000.0;
const HLG_SYSTEM_GAMMA: f32 = 1.2;
const REC2020_LUMA: vec3<f32> = vec3<f32>(0.2627, 0.6780, 0.0593);
// SDR white in an HDR picture, by ITU-R BT.2408.
const REFERENCE_WHITE_NITS: f32 = 203.0;
// Light up to here is kept as it is; brighter rolls off toward white.
const TONE_KNEE: f32 = 0.8;

fn pq_to_light(code: vec3<f32>) -> vec3<f32> {
    let e = pow(clamp(code, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / PQ_M2));
    let light = pow(max(e - vec3<f32>(PQ_C1), vec3<f32>(0.0)) / (vec3<f32>(PQ_C2) - PQ_C3 * e), vec3<f32>(1.0 / PQ_M1));
    return light * (PQ_PEAK_NITS / REFERENCE_WHITE_NITS);
}

fn hlg_to_light(code: vec3<f32>) -> vec3<f32> {
    let c = clamp(code, vec3<f32>(0.0), vec3<f32>(1.0));
    let scene = select((exp((c - vec3<f32>(HLG_C)) / HLG_A) + vec3<f32>(HLG_B)) / 12.0, c * c / 3.0, c <= vec3<f32>(0.5));
    let luma = max(dot(scene, REC2020_LUMA), 1e-6);
    return scene * pow(luma, HLG_SYSTEM_GAMMA - 1.0) * (HLG_PEAK_NITS / REFERENCE_WHITE_NITS);
}

fn to_rec709(rgb: vec3<f32>, primaries: u32) -> vec3<f32> {
    if primaries == 1u {
        return vec3<f32>(
            dot(rgb, vec3<f32>(1.6605, -0.5876, -0.0728)),
            dot(rgb, vec3<f32>(-0.1246, 1.1329, -0.0083)),
            dot(rgb, vec3<f32>(-0.0182, -0.1006, 1.1187)),
        );
    }
    if primaries == 2u {
        return vec3<f32>(
            dot(rgb, vec3<f32>(1.2249, -0.2247, 0.0)),
            dot(rgb, vec3<f32>(-0.0420, 1.0419, 0.0)),
            dot(rgb, vec3<f32>(-0.0197, -0.0786, 1.0979)),
        );
    }
    return rgb;
}

// Rolls the brightest channel off above the knee, keeping hue, so highlights brighter
// than SDR white fit under it.
fn tone_mapped(rgb: vec3<f32>) -> vec3<f32> {
    let brightest = max(rgb.r, max(rgb.g, rgb.b));
    if brightest <= TONE_KNEE {
        return rgb;
    }
    let room = 1.0 - TONE_KNEE;
    let mapped = TONE_KNEE + room * (1.0 - exp(-(brightest - TONE_KNEE) / room));
    return rgb * (mapped / brightest);
}

fn color_managed(rgb: vec3<f32>) -> vec3<f32> {
    let space = u32(placement.key_spill.w + 0.5);
    if space == 0u {
        return rgb;
    }
    // #if key_spill.w
    let transfer = space & 3u;
    var light = rgb;
    if transfer == 1u {
        light = pq_to_light(srgb_encode(rgb));
    } else if transfer == 2u {
        light = hlg_to_light(srgb_encode(rgb));
    }
    return tone_mapped(max(to_rec709(light, space >> 2u), vec3<f32>(0.0)));
    // #else
    return rgb;
    // #endif
}

// The sRGB curve exactly, so a LUT sees the code values the source was decoded from.
fn srgb_encode(rgb: vec3<f32>) -> vec3<f32> {
    let c = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
}

fn srgb_decode(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

fn to_gamma(rgb: vec3<f32>) -> vec3<f32> {
    return pow(max(rgb, vec3<f32>(0.0)), vec3<f32>(1.0 / GAMMA));
}

fn to_linear(rgb: vec3<f32>) -> vec3<f32> {
    return pow(max(rgb, vec3<f32>(0.0)), vec3<f32>(GAMMA));
}

// The colors without the alpha some pictures carry them multiplied by: in linear light
// for composited ones, or on the encoded values for files, which the sampler decoded.
// A frame drawn under the UI is multiplied again, the way the UI blends: on the values
// the display target encodes.
fn unpremultiplied(color: vec4<f32>) -> vec3<f32> {
    let premultiplied = placement.axis_y_opacity.w;
    let alpha = max(color.a, 1e-6);
    if premultiplied > 2.5 {
        return srgb_decode(srgb_encode(color.rgb / alpha) * color.a);
    }
    if premultiplied > 1.5 {
        return srgb_decode(min(srgb_encode(color.rgb) / alpha, vec3<f32>(1.0)));
    }
    if premultiplied > 0.5 {
        return color.rgb / alpha;
    }
    return color.rgb;
}
