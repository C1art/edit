use timeline::{
    ChromaSiting, ChromaSubsampling, PlaneFormat, SampleDepth, YuvColor, YuvMatrix, YuvRange,
};

use super::ChromaLayout;

/// The matrix columns, offset and sample scale, two chroma masks, chroma scale and
/// shift, and footprint and picture size.
pub(super) const UNIFORM_BYTES: usize = 8 * 16;

pub(super) fn uniform(
    color: YuvColor,
    format: PlaneFormat,
    (size, output): ((u32, u32), (u32, u32)),
    (layout, normalized): (ChromaLayout, bool),
) -> [u8; UNIFORM_BYTES] {
    let (columns, offset) = conversion(color, format.depth);
    // Normalized textures already read as 0 to 1; integer ones read what was stored.
    let sample_scale = if normalized {
        1.0
    } else {
        1.0 / most(format.depth)
    };
    let (cb_mask, cr_mask) = match layout {
        ChromaLayout::Separate => ([1.0, 0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0]),
        ChromaLayout::Interleaved => ([1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0]),
    };
    let chroma_scale = match format.subsampling {
        ChromaSubsampling::Yuv420 => [0.5, 0.5],
        ChromaSubsampling::Yuv422 => [0.5, 1.0],
        ChromaSubsampling::Yuv444 => [1.0, 1.0],
    };
    // Left-sited chroma lines up with the left one of each pair of luma pixels, a
    // quarter of a chroma texel left of the pair's middle.
    let shift = match color.siting {
        ChromaSiting::Left if chroma_scale[0] < 1.0 => 0.25,
        ChromaSiting::Left | ChromaSiting::Center => 0.0,
    };
    let footprint = [
        size.0 as f32 / output.0.max(1) as f32,
        size.1 as f32 / output.1.max(1) as f32,
        size.0 as f32,
        size.1 as f32,
    ];
    let [first, second, third] = columns.map(|[r, g, b]| [r, g, b, 0.0]);
    let values: [[f32; 4]; 8] = [
        first,
        second,
        third,
        [offset[0], offset[1], offset[2], sample_scale],
        cb_mask,
        cr_mask,
        [chroma_scale[0], chroma_scale[1], shift, 0.0],
        footprint,
    ];
    let mut bytes = [0; UNIFORM_BYTES];
    let (chunks, _) = bytes.as_chunks_mut::<4>();
    for (chunk, value) in chunks.iter_mut().zip(values.iter().flatten()) {
        *chunk = value.to_le_bytes();
    }
    bytes
}

/// The largest sample `depth` holds.
fn most(depth: SampleDepth) -> f32 {
    ((1u32 << depth.bits()) - 1) as f32
}

/// The matrix's columns from luma, blue- and red-difference to gamma-encoded RGB, and
/// what to subtract from the samples first, both for samples of `depth` scaled to 0 to
/// 1.
fn conversion(color: YuvColor, depth: SampleDepth) -> ([[f32; 3]; 3], [f32; 3]) {
    let (kr, kb) = match color.matrix {
        YuvMatrix::Bt601 => (0.299, 0.114),
        YuvMatrix::Bt709 => (0.2126, 0.0722),
        YuvMatrix::Bt2020 => (0.2627, 0.0593),
    };
    let kg = 1.0 - kr - kb;
    let most = most(depth);
    // Video levels are set in 8-bit steps, which deeper samples have more of.
    let step = (1u32 << (depth.bits() - 8)) as f32;
    let (luma_scale, chroma_scale, black) = match color.range {
        YuvRange::Limited => (
            most / (219.0 * step),
            most / (224.0 * step),
            16.0 * step / most,
        ),
        YuvRange::Full => (1.0, 1.0, 0.0),
    };
    let middle = 128.0 * step / most;
    let cb = [0.0, -2.0 * kb * (1.0 - kb) / kg, 2.0 * (1.0 - kb)];
    let cr = [2.0 * (1.0 - kr), -2.0 * kr * (1.0 - kr) / kg, 0.0];
    let columns = [
        [luma_scale; 3],
        cb.map(|weight| weight * chroma_scale),
        cr.map(|weight| weight * chroma_scale),
    ];
    (columns, [black, middle, middle])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_rgb(color: YuvColor, depth: SampleDepth, yuv: [u16; 3]) -> [f32; 3] {
        let (columns, offset) = conversion(color, depth);
        let sample = yuv.map(|value| f32::from(value) / most(depth));
        let mut rgb = [0.0; 3];
        for ((column, value), offset) in columns.iter().zip(sample).zip(offset) {
            for (channel, weight) in rgb.iter_mut().zip(column) {
                *channel += weight * (value - offset);
            }
        }
        rgb.map(|channel| channel * 255.0)
    }

    fn close(got: [f32; 3], want: [f32; 3]) -> bool {
        got.iter()
            .zip(want)
            .all(|(got, want)| (got - want).abs() < 1.5)
    }

    const VIDEO_709: YuvColor = YuvColor {
        matrix: YuvMatrix::Bt709,
        range: YuvRange::Limited,
        siting: ChromaSiting::Left,
    };

    #[test]
    fn video_levels_span_black_to_white_and_primaries_land_on_their_channel() {
        let eight = SampleDepth::Eight;
        assert!(close(to_rgb(VIDEO_709, eight, [16, 128, 128]), [0.0; 3]));
        assert!(close(to_rgb(VIDEO_709, eight, [235, 128, 128]), [255.0; 3]));
        // BT.709 red at video levels.
        let red = [63, 102, 240];
        assert!(close(to_rgb(VIDEO_709, eight, red), [255.0, 0.0, 0.0]));
        let full = YuvColor {
            matrix: YuvMatrix::Bt601,
            range: YuvRange::Full,
            ..VIDEO_709
        };
        // BT.601 blue at full range, as JPEG has it.
        assert!(close(
            to_rgb(full, eight, [29, 255, 107]),
            [0.0, 0.0, 255.0]
        ));
    }

    #[test]
    fn deeper_samples_scale_video_levels_by_their_extra_bits() {
        for (depth, step) in [(SampleDepth::Ten, 4), (SampleDepth::Twelve, 16)] {
            let at = |yuv: [u16; 3]| to_rgb(VIDEO_709, depth, yuv.map(|value| value * step));
            assert!(close(at([16, 128, 128]), [0.0; 3]), "{depth:?}");
            assert!(close(at([235, 128, 128]), [255.0; 3]), "{depth:?}");
            assert!(close(at([63, 102, 240]), [255.0, 0.0, 0.0]), "{depth:?}");
        }
    }
}
