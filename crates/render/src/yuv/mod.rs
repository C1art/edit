mod converter;
mod uniform;

pub(crate) use converter::{PlaneTextures, YuvConverter};
use timeline::PlaneFormat;

/// One plane's rows, `stride` bytes apart.
#[derive(Debug, Clone, Copy)]
pub struct PlaneRows<'a> {
    pub bytes: &'a [u8],
    pub stride: u32,
}

/// A picture's chroma, at the size its subsampling gives.
#[derive(Debug, Clone, Copy)]
pub enum ChromaRows<'a> {
    /// The blue-difference plane, then the red-difference plane.
    Separate(PlaneRows<'a>, PlaneRows<'a>),
    /// Blue- and red-difference pairs in one plane, as NV12 lays them out.
    Interleaved(PlaneRows<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChromaLayout {
    Separate,
    Interleaved,
}

impl ChromaRows<'_> {
    pub(crate) fn layout(&self) -> ChromaLayout {
        match self {
            Self::Separate(..) => ChromaLayout::Separate,
            Self::Interleaved(_) => ChromaLayout::Interleaved,
        }
    }

    /// Each plane with its samples per texel.
    fn planes(&self) -> impl Iterator<Item = (PlaneRows<'_>, u32)> {
        let (first, second, samples) = match *self {
            Self::Separate(cb, cr) => (cb, Some(cr), 1),
            Self::Interleaved(both) => (both, None, 2),
        };
        std::iter::once((first, samples)).chain(second.map(|cr| (cr, 1)))
    }
}

/// Whether the planes hold a `size` picture in `format`.
pub(crate) fn planes_hold(
    (luma, chroma): (PlaneRows<'_>, &ChromaRows<'_>),
    size: (u32, u32),
    format: PlaneFormat,
) -> bool {
    let bytes = format.depth.bytes();
    let (chroma_width, chroma_height) = format.subsampling.chroma_size(size);
    holds(luma, size.0 * bytes, size.1)
        && chroma
            .planes()
            .all(|(plane, samples)| holds(plane, chroma_width * samples * bytes, chroma_height))
}

/// Whether `plane` holds `height` rows of `row_bytes` each.
fn holds(plane: PlaneRows<'_>, row_bytes: u32, height: u32) -> bool {
    let needed = (plane.stride as usize)
        .checked_mul(height as usize - 1)
        .and_then(|rows| rows.checked_add(row_bytes as usize));
    plane.stride >= row_bytes && needed.is_some_and(|needed| plane.bytes.len() >= needed)
}
