use crate::common::{compositor, layer, solid, SIZE};
use render::{Background, Cdl, LimitBy, Limiter, PixelEffects, RenderError, Tone};
use timeline::BlendMode;

fn rendered(pixels: &[u8], tone: Tone) -> Result<Vec<[u8; 4]>, RenderError> {
    let mut picture = layer(pixels, BlendMode::Normal);
    picture.look.effects = PixelEffects {
        tone,
        ..PixelEffects::NONE
    };
    let mut compositor = compositor();
    compositor.set_background(Background::Transparent);
    compositor.composite((SIZE, SIZE), &[picture])?;
    Ok(compositor.read_output()?.as_chunks::<4>().0.to_vec())
}

fn near(actual: [u8; 4], expected: [u8; 3]) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(&a, e)| a.abs_diff(e) <= 3)
}

#[test]
#[ignore = "needs a GPU"]
fn asc_cdl_applies_slope_offset_and_power_to_encoded_levels() -> Result<(), RenderError> {
    let output = rendered(
        &solid([64; 3]),
        Tone {
            cdl: Some(Cdl {
                slope: [2.0, 1.0, 1.0],
                offset: [0.0, 0.25, 0.0],
                power: [1.0, 1.0, 2.0],
                saturation: 1.0,
            }),
            ..Tone::NONE
        },
    )?;
    // The shader's encoded levels are a 2.2 gamma, not the sRGB curve the picture is
    // stored in, so the 0.26 the grey reads as becomes 0.52, 0.51 and 0.07.
    assert!(near(output[0], [133, 131, 9]), "{:?}", output[0]);
    Ok(())
}

#[test]
#[ignore = "needs a GPU"]
fn a_cdl_power_of_zero_turns_black_white() -> Result<(), RenderError> {
    let output = rendered(
        &solid([0; 3]),
        Tone {
            cdl: Some(Cdl {
                slope: [1.0; 3],
                offset: [0.0; 3],
                power: [0.0; 3],
                saturation: 1.0,
            }),
            ..Tone::NONE
        },
    )?;
    assert!(near(output[0], [255; 3]), "{:?}", output[0]);
    Ok(())
}

#[test]
#[ignore = "needs a GPU"]
fn video_limiter_brings_peaks_down_to_the_clip_level_and_leaves_the_rest() -> Result<(), RenderError>
{
    let pixels: Vec<u8> = (0..SIZE * SIZE)
        .flat_map(|index| match index % SIZE < SIZE / 2 {
            true => [255, 255, 255, 255],
            false => [128, 128, 128, 255],
        })
        .collect();
    let output = rendered(
        &pixels,
        Tone {
            limiter: Some(Limiter {
                level: 0.9,
                knee: 0.0,
                by: LimitBy::Luma,
                warning: None,
            }),
            ..Tone::NONE
        },
    )?;
    assert!(near(output[0], [230; 3]), "{:?}", output[0]);
    assert!(near(output[SIZE as usize - 1], [128; 3]), "{output:?}");
    Ok(())
}
