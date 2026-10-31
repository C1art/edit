use crate::common;
use crate::common::{compositor, filled, layer, solid};
use render::{Detail, PixelEffects, RenderError, UnsharpMask};
use timeline::{AlphaInterpretation, BlendMode, ColorSpace};

fn interpreted(rgba: [u8; 4], alpha: AlphaInterpretation) -> Result<[u8; 3], RenderError> {
    drawn(
        rgba,
        PixelEffects {
            alpha,
            ..PixelEffects::NONE
        },
    )
}

fn drawn(rgba: [u8; 4], effects: PixelEffects) -> Result<[u8; 3], RenderError> {
    let mut compositor = compositor();
    let pixels = filled(rgba);
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.look.effects = effects;
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
fn sharpening_a_flat_hdr_picture_leaves_it_flat() {
    let flat = |detail| {
        let effects = PixelEffects {
            color_space: ColorSpace::Rec2100Pq,
            detail,
            ..PixelEffects::NONE
        };
        drawn([148, 148, 148, 255], effects).unwrap()
    };
    let sharpened = flat(Detail {
        unsharp: Some(UnsharpMask {
            amount: 2.0,
            radius: 2.0,
            threshold: 0.0,
        }),
        ..Detail::NONE
    });
    let plain = flat(Detail::NONE);
    assert!(
        sharpened
            .iter()
            .zip(plain)
            .all(|(&level, plain)| level.abs_diff(plain) <= 1),
        "{sharpened:?} against {plain:?}"
    );
}

#[test]
#[ignore = "needs a GPU"]
fn hdr_reference_white_is_tone_mapped_just_under_sdr_white() {
    let in_space = |code: u8, color_space| {
        let effects = PixelEffects {
            color_space,
            ..PixelEffects::NONE
        };
        drawn([code, code, code, 255], effects).unwrap()
    };
    // 203 nits: PQ's code for it and HLG's 75 percent.
    for shown in [
        in_space(148, ColorSpace::Rec2100Pq),
        in_space(191, ColorSpace::Rec2100Hlg),
    ] {
        assert!(
            shown.iter().all(|&level| level.abs_diff(246) <= 3),
            "got {shown:?}"
        );
    }
    assert_eq!(in_space(148, ColorSpace::Rec709), [148; 3]);
    // Wider primaries leave greys grey.
    let grey = in_space(100, ColorSpace::Rec2020);
    assert!(
        grey.iter().all(|&level| level.abs_diff(100) <= 1),
        "got {grey:?}"
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
