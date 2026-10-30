mod common;

use common::{compositor, layer, solid};
use render::{LayerWipe, RenderError};
use timeline::{BlendMode, Side, Sweep, WipePattern};

/// White wiping in over black: each pixel's red, which is how much of the white shows.
fn wiped(progress: f32, softness: f32) -> Result<Vec<u8>, RenderError> {
    let mut compositor = compositor();
    let (black, white) = (solid([0; 3]), solid([255; 3]));
    let mut incoming = layer(&white, BlendMode::Normal);
    incoming.look.wipes[0] = Some(LayerWipe {
        pattern: WipePattern::Gradient,
        from: Side::Left,
        progress,
        sweep: Sweep::Forward,
        border: 0.0,
        border_color: [1.0; 3],
        softness,
    });
    let pictures = [layer(&black, BlendMode::Normal), incoming];
    compositor.composite((common::SIZE, common::SIZE), &pictures)?;
    Ok(compositor
        .read_output()?
        .chunks(4)
        .map(|pixel| pixel[0])
        .collect())
}

#[test]
#[ignore = "needs a GPU"]
fn a_soft_gradient_wipe_runs_from_nothing_to_everything_through_partial_cover() {
    assert!(wiped(0.0, 0.5).unwrap().iter().all(|&red| red == 0));
    assert!(wiped(1.0, 0.5).unwrap().iter().all(|&red| red == 255));
    let halfway = wiped(0.5, 1.0).unwrap();
    assert!(
        halfway.iter().any(|&red| (20..235).contains(&red)),
        "{halfway:?}"
    );
    let hard = wiped(0.5, 0.0).unwrap();
    assert!(hard.contains(&0) && hard.contains(&255));
}
