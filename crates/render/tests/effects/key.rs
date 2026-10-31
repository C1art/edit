use crate::common;
use crate::common::{compositor, layer, solid};
use render::{ChromaKey, PixelEffects, RenderError};
use timeline::{BlendMode, KeyOutput};

fn ultra_key(output: KeyOutput, spill: f32) -> ChromaKey {
    ChromaKey {
        color: [0.0, 1.0, 0.0],
        tolerance: 0.3,
        softness: 0.1,
        pedestal: 0.0,
        contrast: 0.0,
        choke: 0.0,
        soften: 0.0,
        spill,
        output,
    }
}

fn keyed(rgb: [u8; 3], key: ChromaKey) -> Result<[u8; 3], RenderError> {
    let mut compositor = compositor();
    let pixels = solid(rgb);
    let mut keyed = layer(&pixels, BlendMode::Normal);
    keyed.look.effects = PixelEffects {
        key: Some(key),
        ..PixelEffects::NONE
    };
    compositor.composite((common::SIZE, common::SIZE), &[keyed])?;
    let pixel = compositor.read_output()?;
    Ok([pixel[0], pixel[1], pixel[2]])
}

#[test]
#[ignore = "needs a GPU"]
fn the_alpha_channel_view_shows_the_screen_black_and_the_subject_white() {
    let view = ultra_key(KeyOutput::AlphaChannel, 0.0);
    assert_eq!(keyed([0, 255, 0], view).unwrap(), [0; 3]);
    assert_eq!(keyed([200, 60, 40], view).unwrap(), [255; 3]);
}

#[test]
#[ignore = "needs a GPU"]
fn full_spill_suppression_turns_a_green_cast_grey() {
    let [r, g, b] = keyed([120, 170, 120], ultra_key(KeyOutput::ColorChannel, 1.0)).unwrap();
    assert!(g.abs_diff(r) <= 2 && g.abs_diff(b) <= 2, "got {r} {g} {b}");
}
