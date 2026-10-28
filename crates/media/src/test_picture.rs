use crate::block_match::{Gray, Pyramid};

/// Smooth texture at any point, so a moved picture can be drawn exactly.
pub(crate) fn texture(x: f64, y: f64) -> f64 {
    let hash = |i: i64, j: i64| {
        let n = (i.wrapping_mul(73_856_093) ^ j.wrapping_mul(19_349_663)) as u64;
        (n.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) as f64 / (1u64 << 24) as f64
    };
    let noise = |x: f64, y: f64| {
        let (i, j) = (x.floor(), y.floor());
        let (fx, fy) = (x - i, y - j);
        let (i, j) = (i as i64, j as i64);
        let ease = |t: f64| t * t * (3.0 - 2.0 * t);
        let (sx, sy) = (ease(fx), ease(fy));
        let top = hash(i, j) * (1.0 - sx) + hash(i + 1, j) * sx;
        let bottom = hash(i, j + 1) * (1.0 - sx) + hash(i + 1, j + 1) * sx;
        top * (1.0 - sy) + bottom * sy
    };
    255.0 * (0.6 * noise(x / 6.0, y / 6.0) + 0.4 * noise(x / 2.5, y / 2.5))
}

/// A 320 by 180 picture whose brightness at each point, in square units, `shade` gives.
pub(crate) fn draw(shade: impl Fn([f64; 2]) -> f64) -> Pyramid {
    let (width, height) = (320u32, 180u32);
    let unit = f64::from(width);
    let rgba: Vec<u8> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| {
            let point = [
                (f64::from(x) + 0.5) / unit - 0.5,
                (f64::from(y) + 0.5 - f64::from(height) / 2.0) / unit,
            ];
            let value = shade(point).round() as u8;
            [value, value, value, 255]
        })
        .collect();
    Pyramid::new(Gray::from_rgba(&rgba, width, height))
}
