mod common;

use common::{compositor, layer, solid, SIZE};
use timeline::BlendMode;

/// The encoded value a blend mode gives a backdrop channel under mid grey.
type Expected = fn(u8) -> f32;

#[test]
#[ignore = "needs a GPU"]
fn blend_modes_combine_encoded_values_like_photoshop() {
    let mut compositor = compositor();
    let under = solid([200, 100, 50]);
    let over = solid([128, 128, 128]);
    let product = |b: u8| (f32::from(b) * 128.0 / 255.0).round();
    let screened = |b: u8| 255.0 - ((255.0 - f32::from(b)) * 127.0 / 255.0).round();
    let cases: [(BlendMode, Expected); 4] = [
        (BlendMode::Normal, |_| 128.0),
        (BlendMode::Multiply, product),
        (BlendMode::Screen, screened),
        (BlendMode::Difference, |b| (f32::from(b) - 128.0).abs()),
    ];
    for (mode, expected) in cases {
        let layers = [layer(&under, BlendMode::Normal), layer(&over, mode)];
        compositor.composite((SIZE, SIZE), &layers).unwrap();
        let pixel = compositor.read_output().unwrap();
        for channel in 0..3 {
            let wanted = expected([200, 100, 50][channel]);
            let got = f32::from(pixel[channel]);
            assert!(
                (got - wanted).abs() <= 2.0,
                "{mode:?} channel {channel}: got {got}, wanted {wanted}"
            );
        }
    }
}
