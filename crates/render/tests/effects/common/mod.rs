// Helpers outside `#[test]` fns are not covered by `allow-expect-in-tests`.
#![allow(clippy::expect_used)]

use std::future::Future;
use std::sync::OnceLock;
use std::task::{Context, Poll, Waker};

use render::{Compositor, LayerImage, LayerLook, Picture};
use timeline::BlendMode;

pub const SIZE: u32 = 4;

/// wgpu's native futures are ready when first polled or soon after.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::yield_now();
    }
}

/// One for the whole test binary, as AMD's Vulkan driver can fail, or crash, while
/// two threads set up or use devices of their own at once.
fn device() -> &'static (wgpu::Device, wgpu::Queue) {
    static DEVICE: OnceLock<(wgpu::Device, wgpu::Queue)> = OnceLock::new();
    DEVICE.get_or_init(|| {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("a GPU adapter");
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("a GPU device")
    })
}

pub fn compositor() -> Compositor {
    let (device, queue) = device();
    Compositor::new(device, queue)
}

pub fn solid(rgb: [u8; 3]) -> Vec<u8> {
    filled([rgb[0], rgb[1], rgb[2], 255])
}

pub fn filled(rgba: [u8; 4]) -> Vec<u8> {
    rgba.repeat((SIZE * SIZE) as usize)
}

pub fn layer(rgba: &[u8], blend: BlendMode) -> LayerImage<'_> {
    LayerImage {
        picture: Picture::Pixels {
            width: SIZE,
            height: SIZE,
            rgba,
            source: None,
        },
        look: LayerLook {
            blend,
            ..LayerLook::IDENTITY
        },
        grade: None,
    }
}
