use std::time::Duration;

use crate::common::{compositor, layer, solid, SIZE};
use render::{Background, Compositor, Detail, PixelEffects, Presented, RenderError};
use timeline::BlendMode;

fn sharpened(compositor: &mut Compositor, pixels: &[u8]) -> Result<Presented, RenderError> {
    let mut picture = layer(pixels, BlendMode::Normal);
    picture.look.effects = PixelEffects {
        detail: Detail {
            edge_feather: 1.0,
            ..Detail::NONE
        },
        ..PixelEffects::NONE
    };
    compositor.composite((SIZE, SIZE), &[picture])
}

#[test]
#[ignore = "needs a GPU"]
fn a_frame_left_pending_is_drawn_once_its_shaders_are_built() -> Result<(), RenderError> {
    let pixels = solid([200, 100, 50]);
    let mut inline = compositor();
    inline.set_background(Background::Transparent);
    sharpened(&mut inline, &pixels)?;

    let mut background = compositor();
    background.set_background(Background::Transparent);
    background.compile_in_background();
    let mut presented = sharpened(&mut background, &pixels)?;
    for _ in 0..2_000 {
        if presented != Presented::Pending {
            break;
        }
        while background.compiling() {
            std::thread::sleep(Duration::from_millis(5));
        }
        presented = sharpened(&mut background, &pixels)?;
    }
    // The first frame shown is the first in the texture made for this canvas size.
    assert_eq!(presented, Presented::NewTarget);
    assert_eq!(background.read_output()?, inline.read_output()?);
    Ok(())
}
