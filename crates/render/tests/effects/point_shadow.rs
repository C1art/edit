mod common;

use common::{compositor, layer, SIZE};
use render::{PictureStyle, PixelEffects, PointShadow, RenderError};
use timeline::BlendMode;

#[test]
#[ignore = "needs a GPU"]
fn a_radial_shadow_grows_away_from_its_light() -> Result<(), RenderError> {
    let mut compositor = compositor();
    // Opaque on the left half only.
    let pixels: Vec<u8> = (0..SIZE * SIZE)
        .flat_map(|index| match index % SIZE < SIZE / 2 {
            true => [255, 0, 0, 255],
            false => [0; 4],
        })
        .collect();
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.look.effects = PixelEffects {
        style: PictureStyle {
            shadow_opacity: 1.0,
            shadow_color: [1.0; 3],
            ..PictureStyle::NONE
        },
        point_shadow: Some(PointShadow {
            light: [0.0, 0.5],
            scale: 2.0,
            shadow_only: true,
            glass_influence: None,
        }),
        ..PixelEffects::NONE
    };
    compositor.composite((SIZE, SIZE), &[picture])?;
    let output = compositor.read_output()?;
    let (pixels, _) = output.as_chunks::<4>();
    // Twice as wide from the left edge, the left half's shadow reaches into the
    // transparent right half, and the picture itself is hidden.
    let right = pixels[(SIZE / 2) as usize];
    assert!(right[..3].iter().all(|&channel| channel > 250), "{right:?}");
    let left = pixels[0];
    assert!(left[..3].iter().all(|&channel| channel > 250), "{left:?}");
    Ok(())
}
