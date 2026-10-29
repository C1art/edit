// Kodak's printing density: 0.002 per code value over a negative gamma of 0.6.
const CINEON_DENSITY: f32 = 0.002 / 0.6;

fn cineon_exposure(code: vec3<f32>, white: f32) -> vec3<f32> {
    return pow(vec3<f32>(10.0), (code - white) * CINEON_DENSITY);
}

// Eases what lies above 1 - rolloff toward white so highlights don't clip hard.
fn cineon_rolloff(linear: vec3<f32>, rolloff: f32) -> vec3<f32> {
    if rolloff <= 0.0 {
        return linear;
    }
    let knee = 1.0 - rolloff;
    let eased = knee + rolloff * (1.0 - exp(-(linear - knee) / rolloff));
    return select(linear, eased, linear > vec3<f32>(knee));
}

fn cineon(rgb: vec3<f32>) -> vec3<f32> {
    if placement.cineon_shape.z < 0.5 {
        return rgb;
    }
    return to_linear(cineon_converted(to_gamma(rgb)));
}

// Codes are read from and written as gamma-encoded levels, as the files carry them.
fn cineon_converted(rgb: vec3<f32>) -> vec3<f32> {
    let points = placement.cineon;
    let shape = placement.cineon_shape;
    let black = points.x;
    let white = points.y;
    let range = points.w - points.z;
    let offset = cineon_exposure(vec3<f32>(black), white).x;
    if shape.z < 1.5 {
        let linear = (cineon_exposure(rgb * 1023.0, white) - offset) / (1.0 - offset);
        let shown = pow(max(cineon_rolloff(linear, shape.y), vec3<f32>(0.0)), vec3<f32>(1.0 / shape.x));
        return points.z + range * shown;
    }
    if shape.z < 2.5 {
        let shown = max((rgb - points.z) / max(range, 1e-4), vec3<f32>(0.0));
        let exposure = pow(shown, vec3<f32>(shape.x)) * (1.0 - offset) + offset;
        let code = white + log(max(exposure, vec3<f32>(1e-6))) / log(10.0) / CINEON_DENSITY;
        return code / 1023.0;
    }
    // Log to log moves the given black and white codes to Kodak's standard 95 and 685.
    let code = 95.0 + (rgb * 1023.0 - black) * (590.0 / (white - black));
    return code / 1023.0;
}
