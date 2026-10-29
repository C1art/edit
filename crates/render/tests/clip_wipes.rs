mod common;

use common::{compositor, layer, solid, SIZE};
use render::{ClipWipes, LinearWipe, PixelEffects, RadialSweep, RadialWipe, RenderError};
use timeline::BlendMode;

fn alphas(wipes: ClipWipes) -> Result<Vec<u8>, RenderError> {
    let pixels = solid([255; 3]);
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.look.effects = PixelEffects {
        clip_wipes: wipes,
        ..PixelEffects::NONE
    };
    let mut compositor = compositor();
    compositor.composite((SIZE, SIZE), &[picture])?;
    let output = compositor.read_output()?;
    Ok(output
        .as_chunks::<4>()
        .0
        .iter()
        .map(|pixel| pixel[0])
        .collect())
}

#[test]
#[ignore = "needs a GPU"]
fn a_linear_wipe_at_ninety_degrees_hides_the_left_side_first() -> Result<(), RenderError> {
    let shown = alphas(ClipWipes {
        linear: Some(LinearWipe {
            completion: 0.5,
            angle: std::f32::consts::FRAC_PI_2,
            feather: 0.0,
        }),
        ..ClipWipes::NONE
    })?;
    for row in shown.chunks(SIZE as usize) {
        assert_eq!(row, [0, 0, 255, 255]);
    }
    Ok(())
}

#[test]
#[ignore = "needs a GPU"]
fn a_radial_wipe_both_ways_from_twelve_hides_the_top_half_at_half_completion(
) -> Result<(), RenderError> {
    let shown = alphas(ClipWipes {
        radial: Some(RadialWipe {
            completion: 0.5,
            start_angle: 0.0,
            center: [0.5; 2],
            sweep: RadialSweep::Both,
            feather: 0.0,
        }),
        ..ClipWipes::NONE
    })?;
    let (top, bottom) = shown.split_at(shown.len() / 2);
    assert!(top.iter().all(|&value| value == 0), "{shown:?}");
    assert!(bottom.iter().all(|&value| value == 255), "{shown:?}");
    Ok(())
}
