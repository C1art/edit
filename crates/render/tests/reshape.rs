mod common;

use common::{compositor, layer, solid, SIZE};
use render::{LayerImage, Mirror, PixelEffects, Reshape, Warp};
use timeline::BlendMode;

// Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
#[allow(clippy::unwrap_used)]
fn reshaped(image: LayerImage<'_>, reshape: Reshape) -> Vec<u8> {
    let mut image = image;
    image.look.effects = PixelEffects {
        reshape,
        ..PixelEffects::NONE
    };
    let mut compositor = compositor();
    compositor.composite((SIZE, SIZE), &[image]).unwrap();
    compositor.read_output().unwrap()
}

fn rgb(pixels: &[u8], x: u32, y: u32) -> [u8; 3] {
    let at = ((y * SIZE + x) * 4) as usize;
    [pixels[at], pixels[at + 1], pixels[at + 2]]
}

#[test]
#[ignore = "needs a GPU"]
fn corner_pin_squeezes_the_picture_into_its_corners() {
    let white = solid([255, 255, 255]);
    let mut reshape = Reshape::NONE;
    reshape.push(Warp::CornerPin([
        [0.0, 0.0],
        [0.5, 0.0],
        [0.0, 0.5],
        [0.5, 0.5],
    ]));
    let pixels = reshaped(layer(&white, BlendMode::Normal), reshape);
    assert_eq!(rgb(&pixels, 1, 1), [255, 255, 255]);
    assert_eq!(rgb(&pixels, 2, 0), [0, 0, 0]);
    assert_eq!(rgb(&pixels, 3, 3), [0, 0, 0]);
}

#[test]
#[ignore = "needs a GPU"]
fn a_mirror_shows_the_left_half_on_the_right() {
    let halves: Vec<u8> = (0..SIZE * SIZE)
        .flat_map(|at| {
            if at % SIZE < SIZE / 2 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            }
        })
        .collect();
    let mirror = Some(Mirror {
        center: [0.5, 0.5],
        angle: 0.0,
    });
    let pixels = reshaped(
        layer(&halves, BlendMode::Normal),
        Reshape {
            mirror,
            ..Reshape::NONE
        },
    );
    assert_eq!(rgb(&pixels, 0, 0), [255, 0, 0]);
    assert_eq!(rgb(&pixels, 3, 2), [255, 0, 0]);
}
