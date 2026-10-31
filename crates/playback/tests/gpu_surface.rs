#![cfg(windows)]
// Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use media::{DecodedFrame, MediaReader, PlanarFrame};
use playback::{Layer, LayerPicture};
use render::{Compositor, LayerLook};
use timeline::{Timebase, TrackKind, TrackRef};

/// Decoded pictures that are the same can still come out a level apart, as the
/// hardware decoder's chroma isn't always rounded the same way.
const MOST_DIFFERENT: u8 = 2;

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::yield_now();
    }
}

fn gpu() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::DX12,
        ..wgpu::InstanceDescriptor::new_without_display_handle_from_env()
    });
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .expect("an adapter");
    block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: render::texture_sharing_features(&adapter),
        ..Default::default()
    }))
    .expect("a device")
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

/// The sixth picture of the file, past the first, decoded with the hardware decoder or
/// without.
fn sixth_picture(name: &str, hardware: bool) -> PlanarFrame {
    media::set_hardware_decoding(hardware);
    let mut reader = MediaReader::open(&fixture(name), Timebase::HZ_90K).unwrap();
    reader.set_planar(true);
    let mut pictures =
        std::iter::from_fn(|| reader.next_frame().unwrap()).filter_map(|frame| match frame {
            DecodedFrame::Planar(picture) => Some(picture),
            _ => None,
        });
    pictures.nth(5).expect("six pictures")
}

fn shown(compositor: &mut Compositor, frame: &Arc<PlanarFrame>) -> Vec<u8> {
    let size = (frame.width, frame.height);
    let layer = Layer {
        track: TrackRef {
            kind: TrackKind::Video,
            index: 0,
        },
        picture: LayerPicture::Planes(Arc::clone(frame), size),
        look: LayerLook::IDENTITY,
        grade: None,
    };
    compositor.composite(size, &[layer.image()]).unwrap();
    compositor.read_output().unwrap()
}

#[test]
#[ignore = "needs a GPU that decodes H.264 through D3D12"]
fn pictures_drawn_where_they_were_decoded_match_software_decoding() {
    let (device, queue) = gpu();
    let luid = render::decode_adapter(&device).expect("texture sharing on DX12");
    media::set_decode_adapter(luid);
    let mut compositor = Compositor::new(&device, &queue);
    // The shorter one's height isn't whole macroblocks, so its texture is padded.
    for name in ["testsrc_2s_25fps_44k.mp4", "cuts_1s_1.8s_25fps.mp4"] {
        let software = Arc::new(sixth_picture(name, false));
        let decoded = Arc::new(sixth_picture(name, true));
        assert!(decoded.surface().is_some(), "{name} decoded into a texture");
        let wanted = shown(&mut compositor, &software);
        let drawn = shown(&mut compositor, &decoded);
        let most = wanted
            .iter()
            .zip(&drawn)
            .map(|(wanted, drawn)| wanted.abs_diff(*drawn))
            .max();
        assert!(most <= Some(MOST_DIFFERENT), "{name} differs by {most:?}");
    }
    media::set_hardware_decoding(true);
}
