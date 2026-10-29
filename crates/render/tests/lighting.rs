mod common;

use common::{compositor, layer, solid, SIZE};
use render::{Lamp, LampKind, Lighting, PixelEffects, RenderError};
use timeline::BlendMode;

#[test]
#[ignore = "needs a GPU"]
fn a_spotlight_lights_its_center_and_leaves_the_corners_dark() -> Result<(), RenderError> {
    let mut compositor = compositor();
    let pixels = solid([128; 3]);
    let mut picture = layer(&pixels, BlendMode::Normal);
    let spot = Lamp {
        kind: LampKind::Spotlight,
        color: [1.0; 3],
        center: [0.5; 2],
        major_radius: 0.5,
        minor_radius: 0.5,
        angle: 0.0,
        intensity: 1.0,
        focus: 0.5,
    };
    picture.look.effects = PixelEffects {
        lighting: Some(Lighting {
            lights: [Some(spot), None, None, None, None],
            ambience: [1.0; 3],
            ambience_intensity: 0.0,
            gloss: 0.0,
            material: 0.0,
            exposure: 1.0,
            bump: 0.0,
            white_is_high: true,
        }),
        ..PixelEffects::NONE
    };
    compositor.composite((SIZE, SIZE), &[picture])?;
    let output = compositor.read_output()?;
    let (pixels, _) = output.as_chunks::<4>();
    let center = pixels[(SIZE / 2 * SIZE + SIZE / 2) as usize];
    assert!(center[0] > 100, "{center:?}");
    assert!(
        pixels[0][..3].iter().all(|&channel| channel < 4),
        "{:?}",
        pixels[0]
    );
    Ok(())
}
