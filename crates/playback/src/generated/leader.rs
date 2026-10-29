use std::f32::consts::TAU;

use timeline::{CountingLeader, LeaderPicture, Rgb, TextAlign, Title, TitleFont};

use super::coverage::Coverage;
use super::{draw_title, opaque, Anchor, Canvas};

/// Sizes in fractions of the picture's height.
const LINE_WIDTH: f32 = 1.0 / 180.0;
const RING_WIDTH: f32 = 1.0 / 110.0;
const RING_RADII: [f32; 2] = [0.4, 0.33];
const NUMERAL_SIZE: f32 = 0.5;

pub(super) fn draw_leader(canvas: &mut Canvas, leader: &CountingLeader, picture: LeaderPicture) {
    let LeaderPicture::Count { numeral, swept } = picture else {
        canvas.fill(Rgb::BLACK);
        return;
    };
    let (width, height) = (canvas.width as f32, canvas.height as f32);
    let center = (width / 2.0, height / 2.0);
    let [wipe, background] = [leader.wipe_color, leader.background_color].map(opaque);
    let row = canvas.width as usize;
    for (index, pixel) in canvas.pixels.iter_mut().enumerate() {
        let x = (index % row) as f32 + 0.5 - center.0;
        let y = (index / row) as f32 + 0.5 - center.1;
        // Clockwise from twelve o'clock, as a clock hand sweeps.
        let turned = x.atan2(-y).rem_euclid(TAU) / TAU;
        *pixel = if turned < swept { wipe } else { background };
    }
    let all = (0, 0, i64::from(canvas.width), i64::from(canvas.height));
    let line = height * LINE_WIDTH / 2.0;
    let cross = Coverage::from_fn(row, canvas.height as usize, all, |x, y| {
        let distance = (x - center.0).abs().min((y - center.1).abs());
        (line + 0.5 - distance).clamp(0.0, 1.0)
    });
    canvas.paint(&cross, leader.line_color, 1.0);
    let ring = height * RING_WIDTH / 2.0;
    let rings = Coverage::from_fn(row, canvas.height as usize, all, |x, y| {
        let radius = (x - center.0).hypot(y - center.1);
        let distance = RING_RADII
            .iter()
            .map(|&fraction| (radius - fraction * height).abs())
            .fold(f32::INFINITY, f32::min);
        (ring + 0.5 - distance).clamp(0.0, 1.0)
    });
    canvas.paint(&rings, leader.target_color, 1.0);
    let title = Title {
        text: numeral.to_string(),
        size: (height * NUMERAL_SIZE).round() as u32,
        color: leader.numeral_color,
        align: TextAlign::Center,
        font: TitleFont::Sans,
        shadow: false,
        bold: true,
        ..Title::default()
    };
    draw_title(canvas, &title, Anchor::Center);
}
