mod common;

use common::{compositor, layer};
use render::{ChromaRows, LayerImage, Picture, PlaneRows, RenderError};
use timeline::{BlendMode, ChromaSiting, YuvColor, YuvMatrix, YuvRange};

const WIDTH: u32 = 16;
const HEIGHT: u32 = 4;
/// Each four pixels wide, as luma, blue- and red-difference at video levels.
const STRIPES: [[u8; 3]; 4] = [
    [63, 102, 240],
    [173, 42, 26],
    [32, 240, 118],
    [126, 128, 128],
];

/// The stripes as BT.709 says, in 8-bit RGB.
fn reference() -> Vec<u8> {
    let stripe = |[y, cb, cr]: [u8; 3]| {
        let y = (f32::from(y) - 16.0) / 219.0;
        let (cb, cr) = (
            (f32::from(cb) - 128.0) / 224.0,
            (f32::from(cr) - 128.0) / 224.0,
        );
        let (kr, kb) = (0.2126, 0.0722);
        let kg = 1.0 - kr - kb;
        let r = y + 2.0 * (1.0 - kr) * cr;
        let b = y + 2.0 * (1.0 - kb) * cb;
        let g = (y - kr * r - kb * b) / kg;
        [r, g, b, 1.0].map(|value| (value.clamp(0.0, 1.0) * 255.0).round() as u8)
    };
    let row: Vec<u8> = STRIPES
        .iter()
        .flat_map(|&yuv| stripe(yuv).repeat(4))
        .collect();
    row.repeat(HEIGHT as usize)
}

/// Luma rows padded to `stride`, then the chroma planes, each two samples per stripe.
fn planes(stride: u32) -> (Vec<u8>, [Vec<u8>; 2]) {
    let row = |samples: &dyn Fn([u8; 3]) -> Vec<u8>, stride| {
        let mut row: Vec<u8> = STRIPES.iter().flat_map(|&yuv| samples(yuv)).collect();
        row.resize(stride as usize, 0);
        row
    };
    let luma = row(&|[y, ..]| vec![y; 4], stride).repeat(HEIGHT as usize);
    let half = stride / 2;
    let cb = row(&|[_, cb, _]| vec![cb; 2], half).repeat(HEIGHT as usize / 2);
    let cr = row(&|[.., cr]| vec![cr; 2], half).repeat(HEIGHT as usize / 2);
    (luma, [cb, cr])
}

fn shown(layer: &LayerImage<'_>) -> Result<Vec<u8>, RenderError> {
    let mut compositor = compositor();
    compositor.composite((WIDTH, HEIGHT), std::slice::from_ref(layer))?;
    compositor.read_output()
}

/// The middle two columns of each stripe, the only ones its neighbors' chroma doesn't
/// blend into.
fn stripe_middles(rgba: &[u8]) -> Vec<u8> {
    rgba.as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|(index, _)| matches!(index % 4, 1 | 2))
        .flat_map(|(_, pixel)| pixel.to_vec())
        .collect()
}

#[test]
#[ignore = "needs a GPU"]
fn planes_show_as_the_rgb_they_encode_in_either_layout() -> Result<(), RenderError> {
    let reference = reference();
    let mut rgb = layer(&reference, BlendMode::Normal);
    rgb.picture = Picture::Pixels {
        width: WIDTH,
        height: HEIGHT,
        rgba: &reference,
        source: None,
    };
    let wanted = stripe_middles(&shown(&rgb)?);
    let stride = 24;
    let (luma, [cb, cr]) = planes(stride);
    let interleaved: Vec<u8> = cb.iter().zip(&cr).flat_map(|(&cb, &cr)| [cb, cr]).collect();
    let luma = PlaneRows {
        bytes: &luma,
        stride,
    };
    let chroma_rows = |bytes, stride| PlaneRows { bytes, stride };
    let layouts = [
        ChromaRows::Separate(chroma_rows(&cb, stride / 2), chroma_rows(&cr, stride / 2)),
        ChromaRows::Interleaved(chroma_rows(&interleaved, stride)),
    ];
    for (chroma, siting) in layouts
        .into_iter()
        .zip([ChromaSiting::Left, ChromaSiting::Center])
    {
        let mut image = rgb.clone();
        image.picture = Picture::Planes {
            width: WIDTH,
            height: HEIGHT,
            luma,
            chroma,
            color: YuvColor {
                matrix: YuvMatrix::Bt709,
                range: YuvRange::Limited,
                siting,
            },
            source: None,
        };
        let got = stripe_middles(&shown(&image)?);
        let worst = got
            .iter()
            .zip(&wanted)
            .map(|(got, wanted)| got.abs_diff(*wanted))
            .max();
        assert!(worst <= Some(2), "{got:?} is not {wanted:?}");
    }
    Ok(())
}

#[test]
#[ignore = "needs a GPU"]
fn planes_too_short_for_their_picture_are_refused() {
    let luma = [0; 15];
    let chroma = [128; 8];
    let mut image = layer(&[], BlendMode::Normal);
    image.picture = Picture::Planes {
        width: 4,
        height: 4,
        luma: PlaneRows {
            bytes: &luma,
            stride: 4,
        },
        chroma: ChromaRows::Interleaved(PlaneRows {
            bytes: &chroma,
            stride: 4,
        }),
        color: YuvColor {
            matrix: YuvMatrix::Bt709,
            range: YuvRange::Limited,
            siting: ChromaSiting::Left,
        },
        source: None,
    };
    let refused = compositor().composite((4, 4), &[image]);
    assert!(matches!(refused, Err(RenderError::PlaneSize { .. })));
}
