mod common;

use common::{compositor, filled, layer, solid};
use render::{PixelEffects, RenderError};
use timeline::{AlphaInterpretation, BlendMode};

fn interpreted(rgba: [u8; 4], alpha: AlphaInterpretation) -> Result<[u8; 3], RenderError> {
    let mut compositor = compositor();
    let pixels = filled(rgba);
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.look.effects = PixelEffects {
        alpha,
        ..PixelEffects::NONE
    };
    compositor.composite((common::SIZE, common::SIZE), &[picture])?;
    let pixel = compositor.read_output()?;
    Ok([pixel[0], pixel[1], pixel[2]])
}

#[test]
#[ignore = "needs a GPU"]
fn ignoring_alpha_shows_transparent_pixels_and_inverting_hides_opaque_ones() {
    let hidden = [200, 100, 50, 0];
    assert_eq!(
        interpreted(hidden, AlphaInterpretation::Straight).unwrap(),
        [0; 3]
    );
    let shown = interpreted(hidden, AlphaInterpretation::Ignore).unwrap();
    assert!(
        shown[0].abs_diff(200) <= 1 && shown[2].abs_diff(50) <= 1,
        "got {shown:?}"
    );
    let opaque = [200, 100, 50, 255];
    assert_eq!(
        interpreted(opaque, AlphaInterpretation::Invert).unwrap(),
        [0; 3]
    );
}

#[test]
#[ignore = "needs a GPU"]
fn narrow_pixels_pillarbox_a_square_picture() {
    let mut compositor = compositor();
    let pixels = solid([255; 3]);
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.look.pixel_aspect = 0.5;
    compositor
        .composite((common::SIZE, common::SIZE), &[picture])
        .unwrap();
    let row: Vec<u8> = compositor.read_output().unwrap()[..16]
        .chunks(4)
        .map(|pixel| pixel[0])
        .collect();
    assert_eq!(row, [0, 255, 255, 0]);
}
