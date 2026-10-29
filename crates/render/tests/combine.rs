mod common;

use common::{compositor, layer, solid, SIZE};
use render::{Combine, DifferenceShown, LayerImage, LayerLook, Picture, RenderError};
use timeline::BlendMode;

#[test]
#[ignore = "needs a GPU"]
fn difference_matte_keys_out_only_what_matches_the_other_track() -> Result<(), RenderError> {
    let mut compositor = compositor();
    // Red on the left half, blue on the right, over a plate that is all red.
    let pixels: Vec<u8> = (0..SIZE * SIZE)
        .flat_map(|index| match index % SIZE < SIZE / 2 {
            true => [255, 0, 0, 255],
            false => [0, 0, 255, 255],
        })
        .collect();
    let plate = solid([255, 0, 0]);
    let keyed = LayerImage {
        picture: Picture::Combined {
            layer: vec![layer(&pixels, BlendMode::Normal)],
            other: vec![layer(&plate, BlendMode::Normal)],
            combine: Combine::Difference {
                tolerance: 0.1,
                softness: 0.0,
                blur: 0.0,
                view: DifferenceShown::Keyed,
            },
        },
        look: LayerLook::IDENTITY,
        grade: None,
    };
    compositor.composite((SIZE, SIZE), &[keyed])?;
    let output = compositor.read_output()?;
    let (pixels, _) = output.as_chunks::<4>();
    let [left, right] = [pixels[0], pixels[SIZE as usize - 1]];
    assert!(left[0] < 10 && left[2] < 10, "{left:?}");
    assert!(right[2] > 245, "{right:?}");
    Ok(())
}
