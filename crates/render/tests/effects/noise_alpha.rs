use crate::common;
use crate::common::{compositor, layer, solid};
use render::{AlphaMix, NoisyAlpha, Overflow, PixelEffects, RenderError};
use timeline::BlendMode;

#[test]
#[ignore = "needs a GPU"]
fn clamped_noise_alpha_speckles_an_opaque_picture() -> Result<(), RenderError> {
    let mut compositor = compositor();
    let pixels = solid([255; 3]);
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.look.effects = PixelEffects {
        noise_alpha: Some(NoisyAlpha {
            amount: 1.0,
            squared: false,
            mix: AlphaMix::Clamp,
            overflow: Overflow::Clip,
            seed: 3.0,
            phase: None,
            cycle: None,
        }),
        ..PixelEffects::NONE
    };
    compositor.composite((common::SIZE, common::SIZE), &[picture])?;
    let output = compositor.read_output()?;
    let (pixels, _) = output.as_chunks::<4>();
    let reds = pixels.iter().map(|pixel| pixel[0]);
    let (low, high) = reds.fold((u8::MAX, 0), |(low, high), red| {
        (low.min(red), high.max(red))
    });
    assert!(
        low < 200 && high > 100 && high - low > 40,
        "{low} to {high}"
    );
    Ok(())
}
