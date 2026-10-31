use crate::common::{compositor, layer, SIZE};
use render::{Background, RenderError};
use timeline::BlendMode;

#[test]
#[ignore = "needs a GPU"]
fn a_channel_blur_softens_only_its_channel() -> Result<(), RenderError> {
    // White on the left half, black on the right.
    let pixels: Vec<u8> = (0..SIZE * SIZE)
        .flat_map(|index| {
            let shade = if index % SIZE < SIZE / 2 { 255 } else { 0 };
            [shade, shade, shade, 255]
        })
        .collect();
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.look.blur = [2.0, 0.0, 0.0, 0.0];
    let mut compositor = compositor();
    compositor.set_background(Background::Transparent);
    compositor.composite((SIZE, SIZE), &[picture])?;
    let output = compositor.read_output()?;
    let row: Vec<[u8; 4]> = output.as_chunks::<4>().0[..SIZE as usize].to_vec();
    let edge = SIZE as usize / 2;
    let (white, black) = (row[edge - 1], row[edge]);
    assert!(white[0] < 250 && black[0] > 5, "{row:?}");
    assert_eq!((white[1], black[1]), (255, 0), "{row:?}");
    assert!(row.iter().all(|pixel| pixel[3] == 255), "{row:?}");
    Ok(())
}

#[test]
#[ignore = "needs a GPU"]
fn a_blurred_cutout_keeps_its_color_at_the_edge() -> Result<(), RenderError> {
    // Opaque white on the left half, transparent black on the right.
    let pixels: Vec<u8> = (0..SIZE * SIZE)
        .flat_map(|index| {
            if index % SIZE < SIZE / 2 {
                [255; 4]
            } else {
                [0; 4]
            }
        })
        .collect();
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.look.blur = [2.0; 4];
    let mut compositor = compositor();
    compositor.set_background(Background::Transparent);
    compositor.composite((SIZE, SIZE), &[picture])?;
    let output = compositor.read_output()?;
    let row: Vec<[u8; 4]> = output.as_chunks::<4>().0[..SIZE as usize].to_vec();
    assert!(row.iter().any(|pixel| pixel[3] < 250), "{row:?}");
    assert!(
        row.iter()
            .filter(|pixel| pixel[3] > 8)
            .all(|pixel| pixel[0] > 245),
        "{row:?}"
    );
    Ok(())
}
