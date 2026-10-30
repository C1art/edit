use crate::common::{compositor, filled, layer, SIZE};
use render::{Background, LayerMask, Outline, RenderError, MAX_MASKS};
use timeline::{BlendMode, MaskShape};

fn mask(shape: MaskShape, center: [f32; 2], size: [f32; 2]) -> LayerMask {
    LayerMask {
        shape,
        center,
        size,
        rotation_degrees: 0.0,
        feather: 0.0,
        expansion: 0.0,
        opacity: 1.0,
        inverted: false,
        intersect: false,
        outline: Outline::NONE,
    }
}

/// The alpha of the composited picture's corners: top left, top right, bottom left.
fn corner_alphas(masks: [Option<LayerMask>; MAX_MASKS]) -> Result<[u8; 3], RenderError> {
    let pixels = filled([255; 4]);
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.look.opacity_masks = masks;
    let mut compositor = compositor();
    compositor.set_background(Background::Transparent);
    compositor.composite((SIZE, SIZE), &[picture])?;
    let output = compositor.read_output()?;
    let alpha = |x: u32, y: u32| output[((y * SIZE + x) * 4 + 3) as usize];
    Ok([alpha(0, 0), alpha(SIZE - 1, 0), alpha(0, SIZE - 1)])
}

#[test]
#[ignore = "needs a GPU"]
fn a_garbage_matte_cuts_away_what_the_masks_keep_outside_it() -> Result<(), RenderError> {
    let top_half = [[0.0, 0.0], [1.0, 0.0], [1.0, 0.5], [0.0, 0.5]];
    let matte = LayerMask {
        intersect: true,
        outline: Outline::new(&top_half),
        ..mask(MaskShape::Path, [0.5; 2], [1.0; 2])
    };
    assert_eq!(
        corner_alphas([Some(matte), None, None, None])?,
        [255, 255, 0]
    );
    let left_half = mask(MaskShape::Rectangle, [0.25, 0.5], [0.5, 1.0]);
    let both = [Some(matte), Some(left_half), None, None];
    assert_eq!(corner_alphas(both)?, [255, 0, 0]);
    Ok(())
}
