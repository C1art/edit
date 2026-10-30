mod common;

use common::{compositor, layer, solid};
use render::{ColorAdjust, Creative, CreativeVignette, RenderError};
use timeline::BlendMode;

fn graded(rgb: [u8; 3], creative: Creative) -> Result<Vec<u8>, RenderError> {
    let mut compositor = compositor();
    let pixels = solid(rgb);
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.look.color = ColorAdjust {
        creative,
        ..ColorAdjust::IDENTITY
    };
    compositor.composite((common::SIZE, common::SIZE), &[picture])?;
    compositor.read_output()
}

#[test]
#[ignore = "needs a GPU"]
fn faded_film_lifts_black() {
    let faded = graded(
        [0; 3],
        Creative {
            faded_film: 1.0,
            ..Creative::NONE
        },
    )
    .unwrap();
    assert!((30..50).contains(&faded[0]), "got {:?}", &faded[..4]);
}

#[test]
#[ignore = "needs a GPU"]
fn vignette_darkens_the_corners_and_leaves_the_middle() {
    let vignetted = graded(
        [200; 3],
        Creative {
            vignette: CreativeVignette {
                amount: -1.0,
                ..Creative::NONE.vignette
            },
            ..Creative::NONE
        },
    )
    .unwrap();
    let pixel = |x: u32, y: u32| vignetted[((y * common::SIZE + x) * 4) as usize];
    assert!(pixel(0, 0) < 180, "corner {}", pixel(0, 0));
    assert_eq!(pixel(1, 1), 200);
}
