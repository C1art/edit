mod common;

use common::{compositor, layer, solid, SIZE};
use render::{
    Background, ColorShift, PixelEffects, RenderError, ShiftColor, ShiftMatch, ToneBalance,
};
use timeline::BlendMode;

fn rendered(pixels: &[u8], shift: ColorShift) -> Result<Vec<[u8; 4]>, RenderError> {
    let mut picture = layer(pixels, BlendMode::Normal);
    picture.look.effects = PixelEffects {
        shift,
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
fn change_color_turns_only_the_matched_color() -> Result<(), RenderError> {
    let pixels: Vec<u8> = (0..SIZE * SIZE)
        .flat_map(|index| match index % SIZE < SIZE / 2 {
            true => [255, 0, 0, 255],
            false => [0, 0, 255, 255],
        })
        .collect();
    let output = rendered(
        &pixels,
        ColorShift {
            change_color: Some(ShiftColor {
                target: [1.0, 0.0, 0.0],
                match_by: ShiftMatch::Rgb,
                hue: 1.0 / 3.0,
                lightness: 0.0,
                saturation: 0.0,
                tolerance: 0.15,
                softness: 0.0,
                invert: false,
                show_mask: false,
            }),
            ..ColorShift::NONE
        },
    )?;
    assert!(near(output[0], [0, 255, 0]), "{:?}", output[0]);
    assert!(near(output[SIZE as usize - 1], [0, 0, 255]), "{output:?}");
    Ok(())
}

#[test]
#[ignore = "needs a GPU"]
fn color_balance_pushes_the_midtones_of_a_grey() -> Result<(), RenderError> {
    let output = rendered(
        &solid([128; 3]),
        ColorShift {
            balance: Some(ToneBalance {
                shadows: [0.0; 3],
                midtones: [0.4, 0.0, 0.0],
                highlights: [0.0; 3],
                preserve_luminosity: false,
            }),
            ..ColorShift::NONE
        },
    )?;
    let [r, g, b, _] = output[0];
    assert!(r > 170, "{:?}", output[0]);
    assert!(
        g.abs_diff(128) <= 3 && b.abs_diff(128) <= 3,
        "{:?}",
        output[0]
    );
    Ok(())
}
