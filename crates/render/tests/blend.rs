// Helpers outside `#[test]` fns are not covered by `allow-expect-in-tests`.
#![allow(clippy::expect_used)]

use std::future::Future;
use std::task::{Context, Poll, Waker};

use render::{Compositor, LayerImage, LayerLook, Picture};
use timeline::BlendMode;

const SIZE: u32 = 4;

/// The encoded value a blend mode gives a backdrop channel under mid grey.
type Expected = fn(u8) -> f32;

/// wgpu's native futures are ready when first polled or soon after.
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

fn compositor() -> Compositor {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("a GPU adapter");
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("a GPU device");
    Compositor::new(&device, &queue)
}

fn solid(rgb: [u8; 3]) -> Vec<u8> {
    [rgb[0], rgb[1], rgb[2], 255].repeat((SIZE * SIZE) as usize)
}

fn layer(rgba: &[u8], blend: BlendMode) -> LayerImage<'_> {
    LayerImage {
        picture: Picture::Pixels {
            width: SIZE,
            height: SIZE,
            rgba,
        },
        look: LayerLook {
            blend,
            ..LayerLook::IDENTITY
        },
        grade: None,
    }
}

#[test]
#[ignore = "needs a GPU"]
fn blend_modes_combine_encoded_values_like_photoshop() {
    let mut compositor = compositor();
    let under = solid([200, 100, 50]);
    let over = solid([128, 128, 128]);
    let product = |b: u8| (f32::from(b) * 128.0 / 255.0).round();
    let screened = |b: u8| 255.0 - ((255.0 - f32::from(b)) * 127.0 / 255.0).round();
    let cases: [(BlendMode, Expected); 4] = [
        (BlendMode::Normal, |_| 128.0),
        (BlendMode::Multiply, product),
        (BlendMode::Screen, screened),
        (BlendMode::Difference, |b| (f32::from(b) - 128.0).abs()),
    ];
    for (mode, expected) in cases {
        let layers = [layer(&under, BlendMode::Normal), layer(&over, mode)];
        compositor.composite((SIZE, SIZE), &layers).unwrap();
        let pixel = compositor.read_output().unwrap();
        for channel in 0..3 {
            let wanted = expected([200, 100, 50][channel]);
            let got = f32::from(pixel[channel]);
            assert!(
                (got - wanted).abs() <= 2.0,
                "{mode:?} channel {channel}: got {got}, wanted {wanted}"
            );
        }
    }
}
