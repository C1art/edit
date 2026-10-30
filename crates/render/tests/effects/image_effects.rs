use crate::common;
use crate::common::{compositor, layer, solid};
use render::{
    Cineon, CineonKind, Eyedropper, LeaveColor, Paint, PixelEffects, Recolor, RenderError,
};
use timeline::BlendMode;

fn first_pixel(pixels: &[u8], effects: PixelEffects) -> Result<[u8; 4], RenderError> {
    let mut compositor = compositor();
    let mut picture = layer(pixels, BlendMode::Normal);
    picture.look.effects = effects;
    compositor.composite((common::SIZE, common::SIZE), &[picture])?;
    let output = compositor.read_output()?;
    let (pixels, _) = output.as_chunks::<4>();
    Ok(pixels[0])
}

#[test]
#[ignore = "needs a GPU"]
fn reversed_color_pass_greys_the_matching_color() -> Result<(), RenderError> {
    let pass = LeaveColor {
        color: [1.0, 0.0, 0.0],
        amount: 1.0,
        tolerance: 0.2,
        softness: 0.0,
        by_hue: false,
        reverse: true,
    };
    let effects = PixelEffects {
        recolor: Recolor {
            leave_color: Some(pass),
            ..Recolor::NONE
        },
        ..PixelEffects::NONE
    };
    let [r, g, b, _] = first_pixel(&solid([255, 0, 0]), effects)?;
    assert!(r.abs_diff(g) < 8 && g.abs_diff(b) < 8, "{r} {g} {b}");
    Ok(())
}

#[test]
#[ignore = "needs a GPU"]
fn cineon_log_to_linear_brings_the_white_code_to_white() -> Result<(), RenderError> {
    let effects = PixelEffects {
        cineon: Some(Cineon {
            conversion: CineonKind::LogToLinear,
            black_code: 95.0,
            white_code: 685.0,
            internal_black: 0.0,
            internal_white: 1.0,
            gamma: 1.7,
            highlight_rolloff: 0.0,
        }),
        ..PixelEffects::NONE
    };
    // 171 of 255 is code 686, a hair over white.
    let [r, ..] = first_pixel(&solid([171; 3]), effects)?;
    assert!(r > 245, "{r}");
    Ok(())
}

#[test]
#[ignore = "needs a GPU"]
fn eyedropper_fill_paints_everything_the_sampled_color() -> Result<(), RenderError> {
    // Red on the left half, blue on the right.
    let pixels: Vec<u8> = (0..common::SIZE * common::SIZE)
        .flat_map(|index| match index % common::SIZE < common::SIZE / 2 {
            true => [255, 0, 0, 255],
            false => [0, 0, 255, 255],
        })
        .collect();
    let effects = PixelEffects {
        paint: Paint {
            eyedropper: Some(Eyedropper {
                point: [0.8, 0.5],
                radius: 0.0,
                keep_alpha: true,
                blend: 0.0,
            }),
            ..Paint::NONE
        },
        ..PixelEffects::NONE
    };
    let [r, _, b, _] = first_pixel(&pixels, effects)?;
    assert!(r < 10 && b > 245, "{r} {b}");
    Ok(())
}
