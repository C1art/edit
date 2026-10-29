// Transition wipes (see wipe.rs). Each pattern orders the canvas from 0 to 1 as it
// enters from the left, and a wipe shows what is ordered before its progress.

const WIPE_BANDS: f32 = 8.0;
const WIPE_BLADES: f32 = 8.0;
const WIPE_SQUARES_DOWN: f32 = 8.0;
const WIPE_BLOCKS_DOWN: f32 = 12.0;
const WIPE_SPIRAL_DOWN: f32 = 6.0;
const WIPE_ZIGZAG_DOWN: f32 = 8.0;
// How much chance moves a random wipe's blocks from their place in the sweep.
const WIPE_SCATTER: f32 = 0.35;
// Splatter blotches across the canvas height.
const WIPE_SPLATTER_SCALE: f32 = 4.0;
// Gradient wipe clouds across the canvas height, and how much they outweigh the drift
// from the entry side.
const WIPE_GRADIENT_SCALE: f32 = 1.6;
const WIPE_GRADIENT_CLOUD: f32 = 0.65;
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
        case 13u: { return wipe_hash(floor(pixel / (size.y / WIPE_BLOCKS_DOWN))); }
        case 14u: {
            let across = pixel.x / size.x;
            let odd = fract(floor(pixel.y / size.y * WIPE_BANDS) * 0.5) > 0.25;
            return select(across, 1.0 - across, odd);
        }
        case 15u: {
            let square = size.y / WIPE_SQUARES_DOWN;
            return fract(pixel.x / square + fract(floor(pixel.y / square) * 0.5));
        }
        case 16u: { return wipe_spiral(pixel, size); }
        case 17u: {
            let square = size.y / WIPE_ZIGZAG_DOWN;
            let cells = vec2<f32>(ceil(size.x / square), WIPE_ZIGZAG_DOWN);
            let cell = min(floor(pixel / square), cells - 1.0);
            let down = pixel.y / square - cell.y;
            // Entering from the top, this runs the first row left to right.
            let up = fract(cell.x * 0.5) < 0.25;
            let along = select(cell.y + down, cells.y - cell.y - down, up);
            return (cell.x * cells.y + along) / (cells.x * cells.y);
        }
        case 18u: {
            let block = size.y / WIPE_BLOCKS_DOWN;
            let index = floor(pixel / block);
            return mix(index.x * block / size.x, wipe_hash(index), WIPE_SCATTER);
        }
        case 19u: {
            let point = pixel / size.y * WIPE_SPLATTER_SCALE;
            let noise = 0.6 * wipe_noise(point) + 0.3 * wipe_noise(point * 2.3 + 17.0)
                + 0.1 * wipe_noise(point * 5.1 + 41.0);
            // Value noise gathers around the middle; spread it out to fill the sweep.
            return clamp((noise - 0.25) * 2.0, 0.0, 1.0);
        }
        default: {
            let point = pixel / size.y * WIPE_GRADIENT_SCALE;
            let cloud = 0.7 * wipe_noise(point + 5.0) + 0.3 * wipe_noise(point * 2.7 + 29.0);
            let spread = clamp((cloud - 0.2) * 1.65, 0.0, 1.0);
            return mix(pixel.x / size.x, spread, WIPE_GRADIENT_CLOUD);
        }
    }
}

fn wipe_hash(index: vec2<f32>) -> f32 {
    return fract(sin(dot(index, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

fn wipe_noise(point: vec2<f32>) -> f32 {
    let cell = floor(point);
    let within = fract(point);
    let smooth_within = within * within * (3.0 - 2.0 * within);
    let top = mix(wipe_hash(cell), wipe_hash(cell + vec2<f32>(1.0, 0.0)), smooth_within.x);
    let bottom = mix(
        wipe_hash(cell + vec2<f32>(0.0, 1.0)),
        wipe_hash(cell + vec2<f32>(1.0, 1.0)),
        smooth_within.x,
    );
    return mix(top, bottom, smooth_within.y);
}

// Cells taken clockwise around the outermost ring from the top left, then each ring
// inside it, each cell swept the way the spiral runs through it.
fn wipe_spiral(pixel: vec2<f32>, size: vec2<f32>) -> f32 {
    let cells = vec2<f32>(ceil(size.x / size.y * WIPE_SPIRAL_DOWN), WIPE_SPIRAL_DOWN);
    let scaled = pixel / size * cells;
    let cell = min(floor(scaled), cells - 1.0);
    let within = scaled - cell;
    let ring = min(min(cell.x, cell.y), min(cells.x - 1.0 - cell.x, cells.y - 1.0 - cell.y));
    let inner = cells - 2.0 * ring;
    let outside = cells.x * cells.y - inner.x * inner.y;
    let at = cell - ring;
    let last = inner - 1.0;
    var along: f32;
    if at.y == 0.0 {
        along = at.x + within.x;
    } else if at.x == last.x {
        along = last.x + at.y + within.y;
    } else if at.y == last.y {
        along = last.x + last.y + (last.x - at.x) + 1.0 - within.x;
    } else {
        along = 2.0 * last.x + last.y + (last.y - at.y) + 1.0 - within.y;
    }
    return (outside + along) / (cells.x * cells.y);
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
        let shape = placement.wipes[i * 3u];
        let border = placement.wipes[i * 3u + 1u];
        let softness = placement.wipes[i * 3u + 2u].x;
        let turned = wipe_turn(u32(shape.y), canvas * size, size);
        var order = wipe_order(u32(max(shape.x, 0.0)), turned[0], turned[1]);
        if shape.w > 0.5 {
            order = 1.0 - order;
        }
        let step = min(fwidth(order), WIPE_MAX_STEP);
        if shape.x < 0.0 || shape.z >= 1.0 {
            continue;
        }
        // A soft edge starts wholly before the pattern and ends wholly past it, so the
        // wipe still runs from nothing to everything.
        let progress = shape.z * (1.0 + softness) - 0.5 * softness;
        let width = max(step, softness);
        let edge = progress + border.x * size.y * step;
        let inside = 1.0 - smoothstep(progress - 0.5 * width, progress + 0.5 * width, order);
        let shown = (1.0 - smoothstep(edge - 0.5 * width, edge + 0.5 * width, order))
            * f32(shape.z > 0.0);
        let picture = inside / max(shown, 1e-6);
        if 1.0 - picture > 1.0 - cover.picture {
            cover.border = border.yzw;
        }
        cover.shown *= shown;
        cover.picture *= min(picture, 1.0);
    }
    return cover;
}
