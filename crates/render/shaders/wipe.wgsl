// Transition wipes (see wipe.rs). Each pattern orders the canvas from 0 to 1 as it
// enters from the left, and a wipe shows what is ordered before its progress.

const WIPE_BANDS: f32 = 8.0;
const WIPE_BLADES: f32 = 8.0;
const WIPE_SQUARES_DOWN: f32 = 8.0;
const WIPE_BLOCKS_DOWN: f32 = 12.0;
// Order steps across one pixel beyond this are a pattern's seams, not its edge.
const WIPE_MAX_STEP: f32 = 0.02;

struct WipeCover {
    // How much of the layer the wipes show, and how much of that is picture rather
    // than border.
    shown: f32,
    picture: f32,
    border: vec3<f32>,
}

// Where `pixel` falls in the order `pattern` uncovers a canvas of `size` pixels.
fn wipe_order(pattern: u32, pixel: vec2<f32>, size: vec2<f32>) -> f32 {
    let half = size * 0.5;
    let centered = pixel - half;
    let spread = abs(centered) / half;
    // Turns clockwise from twelve o'clock, -0.5 to 0.5.
    let turn = atan2(centered.x, -centered.y) / (2.0 * PI);
    switch pattern {
        case 0u: { return pixel.x / size.x; }
        case 1u: { return spread.x; }
        case 2u: { return max(spread.x, spread.y); }
        case 3u: { return length(centered) / length(half); }
        case 4u: { return (abs(centered.x) + abs(centered.y)) / (half.x + half.y); }
        case 5u: { return min(spread.x, spread.y); }
        case 6u: { return fract(turn + 1.0); }
        case 7u: { return abs(turn) * 2.0; }
        case 8u: { return atan2(pixel.y, pixel.x) / (0.5 * PI); }
        case 9u: { return fract(fract(turn + 1.0) * WIPE_BLADES); }
        case 10u: { return fract(pixel.x / size.x * WIPE_BANDS); }
        case 11u: {
            let square = size.y / WIPE_SQUARES_DOWN;
            let index = floor(pixel / square);
            let odd = fract((index.x + index.y) * 0.5) * 2.0;
            return (fract(pixel.x / square) + odd) * 0.5;
        }
        case 12u: { return max(pixel.x / size.x, pixel.y / size.y); }
        case 14u: {
            let across = pixel.x / size.x;
            let odd = fract(floor(pixel.y / size.y * WIPE_BANDS) * 0.5) > 0.25;
            return select(across, 1.0 - across, odd);
        }
        default: {
            let index = floor(pixel / (size.y / WIPE_BLOCKS_DOWN));
            return fract(sin(dot(index, vec2<f32>(12.9898, 78.233))) * 43758.5453);
        }
    }
}

// `pixel` on a canvas of `size` turned so that `side` (left, top, right, bottom) is on
// the left: the pixel and the turned canvas's size.
fn wipe_turn(side: u32, pixel: vec2<f32>, size: vec2<f32>) -> array<vec2<f32>, 2> {
    switch side {
        case 1u: { return array<vec2<f32>, 2>(vec2<f32>(pixel.y, size.x - pixel.x), size.yx); }
        case 2u: { return array<vec2<f32>, 2>(size - pixel, size); }
        case 3u: { return array<vec2<f32>, 2>(vec2<f32>(size.y - pixel.y, pixel.x), size.yx); }
        default: { return array<vec2<f32>, 2>(pixel, size); }
    }
}

fn wipe_cover(canvas: vec2<f32>) -> WipeCover {
    // Canvas pixels of the target being drawn.
    let size = 1.0 / max(abs(vec2<f32>(dpdx(canvas.x), dpdy(canvas.y))), vec2<f32>(1e-6));
    var cover = WipeCover(1.0, 1.0, vec3<f32>(0.0));
    for (var i = 0u; i < 2u; i++) {
        let shape = placement.wipes[i * 2u];
        let border = placement.wipes[i * 2u + 1u];
        let turned = wipe_turn(u32(shape.y), canvas * size, size);
        var order = wipe_order(u32(max(shape.x, 0.0)), turned[0], turned[1]);
        if shape.w > 0.5 {
            order = 1.0 - order;
        }
        let step = min(fwidth(order), WIPE_MAX_STEP);
        if shape.x < 0.0 || shape.z >= 1.0 {
            continue;
        }
        let progress = shape.z;
        let edge = progress + border.x * size.y * step;
        let inside = 1.0 - smoothstep(progress - 0.5 * step, progress + 0.5 * step, order);
        let shown = (1.0 - smoothstep(edge - 0.5 * step, edge + 0.5 * step, order))
            * f32(progress > 0.0);
        let picture = inside / max(shown, 1e-6);
        if 1.0 - picture > 1.0 - cover.picture {
            cover.border = border.yzw;
        }
        cover.shown *= shown;
        cover.picture *= min(picture, 1.0);
    }
    return cover;
}
