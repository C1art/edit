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

#[test]
#[ignore = "needs a GPU"]
fn rolling_shutter_repair_shifts_each_line_by_when_it_was_read() {
    let mut compositor = compositor();
    // White in the left half, black in the right.
    let pixels: Vec<u8> = (0..SIZE * SIZE)
        .flat_map(|index| match index % SIZE < SIZE / 2 {
            true => [255; 4],
            false => [0, 0, 0, 255],
        })
        .collect();
    let mut repaired = layer(&pixels, BlendMode::Normal);
    // The top line's middle was read 3/8 of the travel early, the bottom's as late: a
    // quarter of the picture either way, so each reads a whole pixel to the side.
    repaired.look.effects = PixelEffects {
        style: PictureStyle {
            shutter_travel: [2.0 / 3.0, 0.0],
            shutter_scan: [0.0, 1.0],
            ..PictureStyle::NONE
        },
        ..PixelEffects::NONE
    };
    compositor.composite((SIZE, SIZE), &[repaired]).unwrap();
    let output = compositor.read_output().unwrap();
    let red = |x: u32, y: u32| output[((y * SIZE + x) * 4) as usize];
    assert_eq!(red(2, 0), 255);
    assert_eq!(red(1, 3), 0);
}
