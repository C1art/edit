//! The window and taskbar icon, drawn here rather than shipped as an image file: a
//! timeline's clips on two tracks, crossed by the playhead.

use egui::IconData;

const SIZE: u32 = 256;
/// Samples per pixel along each side, for smooth edges.
const SAMPLES: u32 = 4;

const BACKGROUND: [u8; 3] = [30, 31, 34];
const PLAYHEAD: [u8; 3] = [240, 242, 246];

/// A rounded rect in units of the icon's side, with its colour.
struct Shape {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radius: f32,
    color: [u8; 3],
}

const SHAPES: [Shape; 6] = [
    Shape {
        left: 0.02,
        top: 0.02,
        right: 0.98,
        bottom: 0.98,
        radius: 0.2,
        color: BACKGROUND,
    },
    Shape {
        left: 0.14,
        top: 0.24,
        right: 0.52,
        bottom: 0.44,
        radius: 0.05,
        color: [64, 140, 255],
    },
    Shape {
        left: 0.56,
        top: 0.24,
        right: 0.86,
        bottom: 0.44,
        radius: 0.05,
        color: [120, 96, 230],
    },
    Shape {
        left: 0.14,
        top: 0.56,
        right: 0.38,
        bottom: 0.76,
        radius: 0.05,
        color: [60, 190, 140],
    },
    Shape {
        left: 0.42,
        top: 0.56,
        right: 0.86,
        bottom: 0.76,
        radius: 0.05,
        color: [64, 140, 255],
    },
    Shape {
        left: 0.61,
        top: 0.12,
        right: 0.65,
        bottom: 0.88,
        radius: 0.02,
        color: PLAYHEAD,
    },
];

impl Shape {
    fn contains(&self, x: f32, y: f32) -> bool {
        let r = self.radius;
        let nearest_x = x.clamp(self.left + r, self.right - r);
        let nearest_y = y.clamp(self.top + r, self.bottom - r);
        let inside = (self.left..=self.right).contains(&x) && (self.top..=self.bottom).contains(&y);
        inside && (x - nearest_x).powi(2) + (y - nearest_y).powi(2) <= r * r
    }
}

pub(crate) fn icon() -> IconData {
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for row in 0..SIZE {
        for column in 0..SIZE {
            rgba.extend(pixel(column, row));
        }
    }
    IconData {
        rgba,
        width: SIZE,
        height: SIZE,
    }
}

/// The pixel's colour, premultiplied while averaging so edges blend into transparency.
fn pixel(column: u32, row: u32) -> [u8; 4] {
    let mut sum = [0.0f32; 4];
    for sample in 0..SAMPLES * SAMPLES {
        let x = (column as f32 + ((sample % SAMPLES) as f32 + 0.5) / SAMPLES as f32) / SIZE as f32;
        let y = (row as f32 + ((sample / SAMPLES) as f32 + 0.5) / SAMPLES as f32) / SIZE as f32;
        // The topmost shape under the sample is the one seen.
        if let Some(shape) = SHAPES.iter().rev().find(|shape| shape.contains(x, y)) {
            for (total, channel) in sum.iter_mut().zip(shape.color) {
                *total += f32::from(channel);
            }
            sum[3] += 1.0;
        }
    }
    let covered = sum[3];
    if covered == 0.0 {
        return [0; 4];
    }
    let alpha = covered / (SAMPLES * SAMPLES) as f32;
    let [r, g, b, _] = sum.map(|total| (total / covered).round() as u8);
    [r, g, b, (alpha * 255.0).round() as u8]
}
