use timeline::Rgb;

use super::Canvas;

/// 75 % of full scale, the level of SMPTE bars.
const BAR: u8 = 191;
const BLACK: Rgb = Rgb::new(0, 0, 0);
/// Left to right across the top two thirds.
const TOP: [Rgb; 7] = [
    Rgb::new(BAR, BAR, BAR),
    Rgb::new(BAR, BAR, 0),
    Rgb::new(0, BAR, BAR),
    Rgb::new(0, BAR, 0),
    Rgb::new(BAR, 0, BAR),
    Rgb::new(BAR, 0, 0),
    Rgb::new(0, 0, BAR),
];
/// The castellations under them, each the complement of the bar above.
const MIDDLE: [Rgb; 7] = [
    Rgb::new(0, 0, BAR),
    BLACK,
    Rgb::new(BAR, 0, BAR),
    BLACK,
    Rgb::new(0, BAR, BAR),
    BLACK,
    Rgb::new(BAR, BAR, BAR),
];
/// The bottom row as fractions of the width: -I, white, +Q, black, then the PLUGE's
/// below-black, black and above-black strips, then black.
const BOTTOM: [(f32, Rgb); 8] = [
    (5.0 / 28.0, Rgb::new(0, 33, 76)),
    (5.0 / 28.0, Rgb::new(255, 255, 255)),
    (5.0 / 28.0, Rgb::new(50, 0, 106)),
    (5.0 / 28.0, BLACK),
    (1.0 / 21.0, Rgb::new(0, 0, 0)),
    (1.0 / 21.0, Rgb::new(10, 10, 10)),
    (1.0 / 21.0, Rgb::new(20, 20, 20)),
    (1.0 / 7.0, BLACK),
];

/// SMPTE color bars, as Premiere's Bars and Tone draws them.
pub(super) fn draw_bars(canvas: &mut Canvas) {
    let (width, height) = (f64::from(canvas.width), i64::from(canvas.height));
    let x = |fraction: f32| (f64::from(fraction) * width).round() as i64;
    let (top_end, middle_end) = (height * 2 / 3, height * 3 / 4);
    for (index, (top, middle)) in TOP.into_iter().zip(MIDDLE).enumerate() {
        let (left, right) = (x(index as f32 / 7.0), x((index + 1) as f32 / 7.0));
        canvas.fill_box((left, 0, right, top_end), top);
        canvas.fill_box((left, top_end, right, middle_end), middle);
    }
    let mut start = 0.0;
    for (share, color) in BOTTOM {
        let end = start + share;
        canvas.fill_box((x(start), middle_end, x(end), height), color);
        start = end;
    }
}
