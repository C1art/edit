mod common;

use common::{compositor, layer, solid, SIZE};
use render::{Magnify, Optics, PixelEffects};
use timeline::BlendMode;

#[test]
#[ignore = "needs a GPU"]
fn magnify_spreads_the_middle_of_the_picture_over_its_shape() {
    // White down the middle two columns, black at the sides.
    let mut pixels = solid([0; 3]);
    for row in pixels.chunks_mut(SIZE as usize * 4) {
        row[4..12].fill(255);
    }
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.look.effects = PixelEffects {
        optics: Optics {
            magnify: Some(Magnify {
                center: [0.5; 2],
                magnification: 4.0,
                size: 100.0,
                feather: 0.0,
                opacity: 1.0,
                square: false,
            }),
            ..Optics::NONE
        },
        ..PixelEffects::NONE
    };
    let mut compositor = compositor();
    compositor.composite((SIZE, SIZE), &[picture]).unwrap();
    let output = compositor.read_output().unwrap();
    let (pixels, _) = output.as_chunks::<4>();
    assert!(pixels.iter().all(|&[r, g, b, _]| [r, g, b] == [255; 3]));
}
