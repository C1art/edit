use crate::common::{compositor, layer, solid, SIZE};
use render::{Background, Detail, MedianFilter, PixelEffects, RenderError};
use timeline::BlendMode;

fn rendered(pixels: &[u8], detail: Detail) -> Result<Vec<[u8; 4]>, RenderError> {
    let mut picture = layer(pixels, BlendMode::Normal);
    picture.look.effects = PixelEffects {
        detail,
        ..PixelEffects::NONE
    };
    let mut compositor = compositor();
    compositor.set_background(Background::Transparent);
    compositor.composite((SIZE, SIZE), &[picture])?;
    Ok(compositor.read_output()?.as_chunks::<4>().0.to_vec())
}

#[test]
#[ignore = "needs a GPU"]
fn a_median_takes_out_a_lone_speck() -> Result<(), RenderError> {
    let mut pixels = solid([0; 3]);
    let speck = (SIZE as usize + 1) * 4;
    pixels[speck..speck + 3].copy_from_slice(&[255; 3]);
    let output = rendered(
        &pixels,
        Detail {
            median: Some(MedianFilter {
                radius: 1.0,
                threshold: 0.0,
            }),
            ..Detail::NONE
        },
    )?;
    assert!(
        output.iter().all(|pixel| pixel[..3] == [0; 3]),
        "{output:?}"
    );
    Ok(())
}

#[test]
#[ignore = "needs a GPU"]
fn edge_feather_fades_the_picture_toward_its_edges() -> Result<(), RenderError> {
    let pixels = solid([255; 3]);
    let output = rendered(
        &pixels,
        Detail {
            edge_feather: 2.0,
            ..Detail::NONE
        },
    )?;
    let alphas: Vec<u8> = output.iter().map(|pixel| pixel[3]).collect();
    // Pixel centers half a pixel in show a quarter, those a pixel and a half in three
    // quarters.
    let expected = [
        64, 64, 64, 64, 64, 191, 191, 64, 64, 191, 191, 64, 64, 64, 64, 64,
    ];
    let close = alphas
        .iter()
        .zip(expected)
        .all(|(&alpha, expected)| alpha.abs_diff(expected) <= 2);
    assert!(close, "{alphas:?}");
    Ok(())
}
