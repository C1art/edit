use crate::common::{compositor, layer};
use render::{ChromaRows, LayerImage, Picture, PlaneRows, RenderError};
use timeline::{
    BlendMode, ChromaSiting, ChromaSubsampling, PlaneFormat, SampleDepth, YuvColor, YuvMatrix,
    YuvRange,
};

const WIDTH: u32 = 16;
const HEIGHT: u32 = 4;
const EIGHT_BIT_420: PlaneFormat = PlaneFormat {
    subsampling: ChromaSubsampling::Yuv420,
    depth: SampleDepth::Eight,
};
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

/// The stripes in `format`, rows packed tight: luma, then each chroma plane.
fn planes_in(format: PlaneFormat) -> [(Vec<u8>, u32); 3] {
    let (chroma_width, chroma_height) = format.subsampling.chroma_size((WIDTH, HEIGHT));
    let per_stripe = chroma_width / 4;
    let shift = format.depth.bits() - 8;
    let plane = |pick: fn([u8; 3]) -> u8, width: u32, height: u32| {
        let row: Vec<u8> = STRIPES
            .iter()
            .flat_map(|&yuv| vec![u16::from(pick(yuv)) << shift; (width / 4) as usize])
            .flat_map(|sample| match format.depth {
                SampleDepth::Eight => vec![sample as u8],
                _ => sample.to_le_bytes().to_vec(),
            })
            .collect();
        let stride = row.len() as u32;
        (row.repeat(height as usize), stride)
    };
    [
        plane(|[y, ..]| y, WIDTH, HEIGHT),
        plane(|[_, cb, _]| cb, per_stripe * 4, chroma_height),
        plane(|[.., cr]| cr, per_stripe * 4, chroma_height),
    ]
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
            output: (WIDTH, HEIGHT),
            luma,
            chroma,
            format: EIGHT_BIT_420,
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
fn planes_of_any_subsampling_and_depth_show_the_same_rgb() -> Result<(), RenderError> {
    #[derive(Debug)]
    enum Layout {
        Separate,
        Interleaved,
    }
    use Layout::*;
    let reference = reference();
    let mut rgb = layer(&reference, BlendMode::Normal);
    rgb.picture = Picture::Pixels {
        width: WIDTH,
        height: HEIGHT,
        rgba: &reference,
        source: None,
    };
    let wanted = stripe_middles(&shown(&rgb)?);
    let formats = [
        (ChromaSubsampling::Yuv422, SampleDepth::Eight, Separate),
        (ChromaSubsampling::Yuv444, SampleDepth::Eight, Separate),
        (ChromaSubsampling::Yuv420, SampleDepth::Ten, Separate),
        (ChromaSubsampling::Yuv422, SampleDepth::Twelve, Separate),
        (ChromaSubsampling::Yuv444, SampleDepth::Sixteen, Separate),
        (ChromaSubsampling::Yuv420, SampleDepth::Sixteen, Interleaved),
    ];
    for (subsampling, depth, layout) in formats {
        let format = PlaneFormat { subsampling, depth };
        let [(luma, luma_stride), (cb, cb_stride), (cr, cr_stride)] = planes_in(format);
        let size = depth.bytes() as usize;
        let interleaved: Vec<u8> = cb
            .chunks(size)
            .zip(cr.chunks(size))
            .flat_map(|(cb, cr)| [cb, cr].concat())
            .collect();
        let rows = |bytes, stride| PlaneRows { bytes, stride };
        let chroma = match layout {
            Separate => ChromaRows::Separate(rows(&cb, cb_stride), rows(&cr, cr_stride)),
            Interleaved => ChromaRows::Interleaved(rows(&interleaved, cb_stride * 2)),
        };
        let mut image = rgb.clone();
        image.picture = Picture::Planes {
            width: WIDTH,
            height: HEIGHT,
            output: (WIDTH, HEIGHT),
            luma: rows(&luma, luma_stride),
            chroma,
            format,
            color: YuvColor {
                matrix: YuvMatrix::Bt709,
                range: YuvRange::Limited,
                siting: ChromaSiting::Left,
            },
            source: None,
        };
        let got = stripe_middles(&shown(&image)?);
        let worst = got
            .iter()
            .zip(&wanted)
            .map(|(got, wanted)| got.abs_diff(*wanted))
            .max();
        assert!(
            worst <= Some(2),
            "{format:?} {layout:?}: {got:?} is not {wanted:?}"
        );
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
        output: (4, 4),
        luma: PlaneRows {
            bytes: &luma,
            stride: 4,
        },
        chroma: ChromaRows::Interleaved(PlaneRows {
            bytes: &chroma,
            stride: 4,
        }),
        format: EIGHT_BIT_420,
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

#[test]
#[ignore = "needs a GPU"]
fn shrunk_planes_average_the_pixels_each_output_pixel_covers() -> Result<(), RenderError> {
    let (width, height) = (8, 4);
    // Columns of video black and white, which only averaging all of each two or four
    // turns mid-gray.
    let luma: Vec<u8> = [16, 235, 235, 16].repeat(width as usize / 4 * height as usize);
    let chroma = [128; 8];
    let half = PlaneRows {
        bytes: &chroma,
        stride: width / 2,
    };
    for output in [(4, 2), (2, 1)] {
        let mut image = layer(&[], BlendMode::Normal);
        image.picture = Picture::Planes {
            width,
            height,
            output,
            luma: PlaneRows {
                bytes: &luma,
                stride: width,
            },
            chroma: ChromaRows::Separate(half, half),
            format: EIGHT_BIT_420,
            color: YuvColor {
                matrix: YuvMatrix::Bt709,
                range: YuvRange::Limited,
                siting: ChromaSiting::Center,
            },
            source: None,
        };
        let mut compositor = compositor();
        compositor.composite((width, height), &[image])?;
        let shown = compositor.read_output()?;
        let gray = shown.as_chunks::<4>().0.iter().all(|pixel| {
            pixel[..3]
                .iter()
                .all(|channel| (126..=129).contains(channel))
                && pixel[3] == 255
        });
        assert!(gray, "shrunk to {output:?}: {shown:?}");
    }
    Ok(())
}
