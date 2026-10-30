use std::any::Any;
use std::sync::Arc;

use crate::common::{compositor, layer, solid, SIZE};
use render::{Background, Picture, RenderError};
use timeline::BlendMode;

#[test]
#[ignore = "needs a GPU"]
fn a_layer_is_uploaded_again_only_when_its_source_changes() -> Result<(), RenderError> {
    let red: Arc<Vec<u8>> = Arc::new(solid([255, 0, 0]));
    let blue: Arc<Vec<u8>> = Arc::new(solid([0, 0, 255]));
    let mut compositor = compositor();
    compositor.set_background(Background::Transparent);
    let mut show = |pixels: &Arc<Vec<u8>>, source: &Arc<Vec<u8>>| {
        let mut image = layer(pixels, BlendMode::Normal);
        image.picture = Picture::Pixels {
            width: SIZE,
            height: SIZE,
            rgba: pixels,
            source: Some(Arc::clone(source) as Arc<dyn Any + Send + Sync>),
        };
        compositor.composite((SIZE, SIZE), &[image])?;
        compositor.read_output()
    };
    let shown_red = show(&red, &red)?;
    let shown_blue = show(&blue, &blue)?;
    assert_ne!(shown_red, shown_blue);
    // Claiming red's source for blue's pixels shows what red's upload left.
    assert_eq!(show(&red, &red)?, shown_red);
    assert_eq!(show(&blue, &red)?, shown_red);
    Ok(())
}
