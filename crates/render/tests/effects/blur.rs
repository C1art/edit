use crate::common::{compositor, layer, SIZE};
use render::{Background, Picture, RenderError};
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

#[test]
#[ignore = "needs a GPU"]
fn a_wide_blur_spreads_a_thin_line_smoothly() -> Result<(), RenderError> {
    const WIDTH: u32 = 256;
    // One white column on black.
    let pixels: Vec<u8> = (0..WIDTH * 2)
        .flat_map(|index| {
            let shade = if index % WIDTH == WIDTH / 2 { 255 } else { 0 };
            [shade, shade, shade, 255]
        })
        .collect();
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.picture = Picture::Pixels {
        width: WIDTH,
        height: 2,
        rgba: &pixels,
        source: None,
    };
    picture.look.blur = [32.0; 4];
    let mut compositor = compositor();
    compositor.composite((WIDTH, 2), &[picture])?;
    let output = compositor.read_output()?;
    let row: Vec<u8> = output.as_chunks::<4>().0[..WIDTH as usize]
        .iter()
        .map(|pixel| pixel[0])
        .collect();
    let middle = &row[WIDTH as usize / 2 - 8..WIDTH as usize / 2 + 8];
    assert!(middle.iter().all(|&level| level > 0), "{middle:?}");
    assert!(
        middle.windows(2).all(|pair| pair[0].abs_diff(pair[1]) <= 2),
        "{middle:?}"
    );
    Ok(())
}

#[test]
#[ignore = "needs a GPU"]
fn a_squashed_layer_blurs_alike_both_ways_on_the_canvas() -> Result<(), RenderError> {
    const SIDE: u32 = 64;
    // A white square in the middle of black.
    let pixels: Vec<u8> = (0..SIDE * SIDE)
        .flat_map(|index| {
            let (x, y) = (index % SIDE, index / SIDE);
            let inside = (30..34).contains(&x) && (30..34).contains(&y);
            let shade = if inside { 255 } else { 0 };
            [shade, shade, shade, 255]
        })
        .collect();
    let mut picture = layer(&pixels, BlendMode::Normal);
    picture.picture = Picture::Pixels {
        width: SIDE,
        height: SIDE,
        rgba: &pixels,
        source: None,
    };
    picture.look.scale = [1.0, 0.5];
    picture.look.blur = [3.0; 4];
    let mut compositor = compositor();
    compositor.composite((SIDE, SIDE), &[picture])?;
    let output = compositor.read_output()?;
    let pixels = output.as_chunks::<4>().0;
    let lit = |pixel: &[u8; 4]| pixel[0] > 4;
    let across = pixels[(SIDE * SIDE / 2) as usize..][..SIDE as usize]
        .iter()
        .filter(|pixel| lit(pixel))
        .count();
    let down = pixels
        .iter()
        .skip(SIDE as usize / 2)
        .step_by(SIDE as usize)
        .filter(|pixel| lit(pixel))
        .count();
    // The square is half as tall as wide once squashed; its blur reaches as far.
    assert!(
        across.abs_diff(down + 2) <= 2,
        "{across} across, {down} down"
    );
    Ok(())
}
