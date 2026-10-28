mod common;

use common::{compositor, layer, solid};
use render::{Adjust, LumaKey, PixelEffects, RenderError};
use timeline::BlendMode;

fn adjusted(rgb: [u8; 3], adjust: Adjust) -> Result<[u8; 3], RenderError> {
    let mut compositor = compositor();
    let pixels = solid(rgb);
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.look.effects = PixelEffects {
        adjust,
        ..PixelEffects::NONE
    };
    compositor.composite((common::SIZE, common::SIZE), &[picture])?;
    let pixel = compositor.read_output()?;
    Ok([pixel[0], pixel[1], pixel[2]])
}

#[test]
#[ignore = "needs a GPU"]
fn luma_key_fades_from_cutoff_to_threshold() {
    let key = |threshold, cutoff| Adjust {
        luma_key: Some(LumaKey { threshold, cutoff }),
        ..Adjust::NONE
    };
    assert_eq!(adjusted([40; 3], key(0.5, 0.2)).unwrap(), [0; 3]);
    assert_eq!(adjusted([220; 3], key(0.5, 0.2)).unwrap(), [220; 3]);
    // A cutoff above the threshold keys out the brights instead.
    assert_eq!(adjusted([220; 3], key(0.2, 0.5)).unwrap(), [0; 3]);
}

#[test]
#[ignore = "needs a GPU"]
fn solarize_inverts_only_channels_above_its_level() {
    let solarized = adjusted(
        [200, 100, 30],
        Adjust {
            solarize: Some(0.5),
            ..Adjust::NONE
        },
    )
    .unwrap();
    let expected = [55, 100, 30];
    assert!(
        solarized
            .iter()
            .zip(expected)
            .all(|(got, want)| got.abs_diff(want) <= 2),
        "got {solarized:?}"
    );
}
