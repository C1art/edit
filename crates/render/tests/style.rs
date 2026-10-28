mod common;

use common::{compositor, layer, solid, SIZE};
use render::{LayerLook, PictureStyle, PixelEffects};
use timeline::BlendMode;

#[test]
#[ignore = "needs a GPU"]
fn a_drop_shadow_falls_past_the_picture_it_is_cast_by() {
    let mut compositor = compositor();
    let white = solid([255, 255, 255]);
    let mut shrunk = layer(&white, BlendMode::Normal);
    // Half size in the middle of the canvas, so the picture covers pixels 1 and 2 each
    // way, and its shadow falls a canvas pixel right and down.
    shrunk.look = LayerLook {
        scale: [0.5; 2],
        effects: PixelEffects {
            style: PictureStyle {
                shadow_offset: [2.0, 2.0],
                shadow_opacity: 1.0,
                shadow_color: [1.0, 0.0, 0.0],
                ..PictureStyle::NONE
            },
            ..PixelEffects::NONE
        },
        ..shrunk.look
    };
    compositor.composite((SIZE, SIZE), &[shrunk]).unwrap();
    let pixels = compositor.read_output().unwrap();
    let rgb = |x: u32, y: u32| {
        let at = ((y * SIZE + x) * 4) as usize;
        [pixels[at], pixels[at + 1], pixels[at + 2]]
    };
    assert_eq!(rgb(2, 2), [255, 255, 255]);
    assert_eq!(rgb(3, 3), [255, 0, 0]);
    assert_eq!(rgb(3, 2), [255, 0, 0]);
    assert_eq!(rgb(0, 0), [0, 0, 0]);
    assert_eq!(rgb(3, 1), [0, 0, 0]);
}
