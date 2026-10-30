use crate::common::{compositor, layer, solid, SIZE};
use render::{EllipseRing, Pattern, PixelEffects, RenderError};
use timeline::BlendMode;

#[test]
#[ignore = "needs a GPU"]
fn ellipse_draws_its_ring_over_black_in_place_of_the_picture() -> Result<(), RenderError> {
    let mut compositor = compositor();
    let pixels = solid([128; 3]);
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.look.effects = PixelEffects {
        pattern: Pattern {
            ellipse: Some(EllipseRing {
                center: [0.5; 2],
                size: [SIZE as f32; 2],
                thickness: 1.0,
                softness: 0.0,
                inside: [0.0, 1.0, 0.0],
                outside: [0.0, 1.0, 0.0],
                over_picture: false,
            }),
            ..Pattern::NONE
        },
        ..PixelEffects::NONE
    };
    compositor.composite((SIZE, SIZE), &[picture])?;
    let output = compositor.read_output()?;
    let (pixels, _) = output.as_chunks::<4>();
    // The corner pixel's center sits on the ring; the one inside it is well clear.
    let [r, g, b, _] = pixels[0];
    assert!(r < 8 && g > 230 && b < 8, "{:?}", pixels[0]);
    let inside = pixels[(SIZE + 1) as usize];
    assert!(inside[..3].iter().all(|&channel| channel < 4), "{inside:?}");
    Ok(())
}
