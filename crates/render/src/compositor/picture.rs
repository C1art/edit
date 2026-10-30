use std::any::Any;
use std::sync::Arc;

use timeline::{MatteChannel, PlaneFormat, YuvColor};

use super::LayerImage;
use crate::combine::Combine;
use crate::yuv::{ChromaRows, PlaneRows};

#[derive(Debug, Clone)]
pub enum Picture<'a> {
    /// Tightly packed RGBA8 in the source transfer function.
    Pixels {
        width: u32,
        height: u32,
        rgba: &'a [u8],
        /// What holds `rgba`, unchanged for as long as it lives. A layer showing the
        /// same source as last time keeps its texture instead of uploading it again.
        source: Option<Arc<dyn Any + Send + Sync>>,
    },
    /// A picture's planes, in the source transfer function, turned into RGB on the
    /// GPU.
    Planes {
        width: u32,
        height: u32,
        /// The size to turn them into: theirs, or smaller to shrink them on the way.
        output: (u32, u32),
        luma: PlaneRows<'a>,
        chroma: ChromaRows<'a>,
        color: YuvColor,
        format: PlaneFormat,
        /// As for [`Picture::Pixels`].
        source: Option<Arc<dyn Any + Send + Sync>>,
    },
    /// A picture decoded with D3D12, in the source transfer function, turned into RGB
    /// straight from the decoder's NV12 or P010 texture.
    #[cfg(windows)]
    Surface {
        width: u32,
        height: u32,
        /// As for [`Picture::Planes`].
        output: (u32, u32),
        /// Decoded to its end, and unchanged for as long as `source` lives.
        texture: &'a windows::Win32::Graphics::Direct3D12::ID3D12Resource,
        color: YuvColor,
        /// Eight bits a sample for NV12, sixteen for P010.
        format: PlaneFormat,
        /// As for [`Picture::Pixels`], but kept until the GPU is done reading the
        /// texture, which its decoder reuses once it's let go.
        source: Option<Arc<dyn Any + Send + Sync>>,
    },
    /// Layers composited bottom first onto a transparent canvas of their own, so the
    /// look applies to them as one picture, as to a nested sequence.
    Group(Vec<LayerImage<'a>>),
    /// `fill` composited as a group, shown only where the group `matte` lets it.
    Matted {
        fill: Vec<LayerImage<'a>>,
        matte: Vec<LayerImage<'a>>,
        channel: MatteChannel,
        reverse: bool,
    },
    /// `layer` composited as a group, combined with the group `other`.
    Combined {
        layer: Vec<LayerImage<'a>>,
        other: Vec<LayerImage<'a>>,
        combine: Combine,
    },
}
