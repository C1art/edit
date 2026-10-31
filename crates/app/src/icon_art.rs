//! The Cledit icon, drawn from shapes rather than shipped as an image file so the
//! window icon and the exe's icon (made by build.rs) come from one design: three
//! tracks of clips forming a "C", crossed by the playhead.

/// Samples per pixel along each side, for smooth edges.
const SAMPLES: u32 = 4;

/// A rounded rect in units of the icon's side, with its colour.
struct Shape {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radius: f32,
    color: [u8; 3],
}

const SHAPES: [Shape; 5] = [
    Shape {
        left: 0.02,
        top: 0.02,
        right: 0.98,
        bottom: 0.98,
        radius: 0.2,
        color: [30, 31, 34],
    },
    Shape {
        left: 0.16,
        top: 0.2,
        right: 0.84,
        bottom: 0.37,
        radius: 0.05,
        color: [64, 140, 255],
    },
    Shape {
        left: 0.16,
        top: 0.415,
        right: 0.42,
        bottom: 0.585,
        radius: 0.05,
        color: [60, 190, 140],
    },
    Shape {
        left: 0.16,
        top: 0.63,
        right: 0.84,
        bottom: 0.8,
        radius: 0.05,
        color: [120, 96, 230],
    },
    Shape {
        left: 0.635,
        top: 0.11,
        right: 0.685,
        bottom: 0.89,
        radius: 0.025,
        color: [240, 242, 246],
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

/// The icon at `size` pixels square, as straight-alpha RGBA rows from the top.
pub fn rgba(size: u32) -> Vec<u8> {
    (0..size)
        .flat_map(|row| (0..size).flat_map(move |column| pixel(column, row, size)))
        .collect()
}

fn pixel(column: u32, row: u32, size: u32) -> [u8; 4] {
    let mut sum = [0.0f32; 4];
    for sample in 0..SAMPLES * SAMPLES {
        let x = (column as f32 + ((sample % SAMPLES) as f32 + 0.5) / SAMPLES as f32) / size as f32;
        let y = (row as f32 + ((sample / SAMPLES) as f32 + 0.5) / SAMPLES as f32) / size as f32;
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
