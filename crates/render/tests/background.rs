mod common;

use common::{compositor, layer, solid, SIZE};
use render::{Background, LayerLook};
use timeline::BlendMode;

#[test]
#[ignore = "needs a GPU"]
fn a_transparent_background_keeps_uncovered_pixels_clear() {
    let mut compositor = compositor();
    compositor.set_background(Background::Transparent);
    let red = solid([255, 0, 0]);
    let mut shrunk = layer(&red, BlendMode::Normal);
    // Covers pixels 1 and 2 each way, half see-through.
    shrunk.look = LayerLook {
        scale: [0.5; 2],
        opacity: 0.5,
        ..shrunk.look
    };
    compositor.composite((SIZE, SIZE), &[shrunk]).unwrap();
    let pixels = compositor.read_output().unwrap();
    let rgba = |x: u32, y: u32| {
        let at = ((y * SIZE + x) * 4) as usize;
        [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
    };
    assert_eq!(rgba(0, 0)[3], 0);
    let [r, g, b, a] = rgba(1, 1);
    // Straight color, not darkened by the half coverage.
    assert_eq!([r, g, b], [255, 0, 0]);
    assert!((a as i32 - 128).abs() <= 1, "{a}");
}
