mod common;

use common::{compositor, layer, solid, SIZE};
use render::{Checkerboard, FourColorGradient, Generate, PixelEffects, RenderError};
use timeline::BlendMode;

/// Each output pixel's red, green and blue, row by row.
fn generated(generate: Generate) -> Result<Vec<[u8; 3]>, RenderError> {
    let mut compositor = compositor();
    let pixels = solid([0; 3]);
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.look.effects = PixelEffects {
        generate,
        ..PixelEffects::NONE
    };
    compositor.composite((SIZE, SIZE), &[picture])?;
    let output = compositor.read_output()?;
    let (pixels, _) = output.as_chunks::<4>();
    Ok(pixels.iter().map(|&[r, g, b, _]| [r, g, b]).collect())
}

#[test]
#[ignore = "needs a GPU"]
fn checkerboard_fills_every_other_square_from_its_anchor() {
    let pixels = generated(Generate {
        checkerboard: Some(Checkerboard {
            anchor: [0.0; 2],
            size: [1.0; 2],
            color: [1.0; 3],
            opacity: 1.0,
        }),
        ..Generate::NONE
    })
    .unwrap();
    let row = SIZE as usize;
    assert_eq!(pixels[0], [255; 3]);
    assert_eq!(pixels[1], [0; 3]);
    assert_eq!(pixels[row], [0; 3]);
    assert_eq!(pixels[row + 1], [255; 3]);
}

#[test]
#[ignore = "needs a GPU"]
fn four_color_gradient_takes_each_points_color_at_it() {
    // Each point sits on a corner pixel's center.
    let near = 0.5 / SIZE as f32;
    let far = 1.0 - near;
    let pixels = generated(Generate {
        four_color: Some(FourColorGradient {
            points: [[near, near], [far, near], [near, far], [far, far]],
            colors: [
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [1.0, 1.0, 1.0],
            ],
            blend: 1.0,
            opacity: 1.0,
        }),
        ..Generate::NONE
    })
    .unwrap();
    let last = (SIZE * SIZE) as usize - 1;
    assert_eq!(pixels[0], [255, 0, 0]);
    assert_eq!(pixels[SIZE as usize - 1], [0, 255, 0]);
    assert_eq!(pixels[last], [255; 3]);
}
