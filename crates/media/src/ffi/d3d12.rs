//! Decoding with D3D12 on the adapter that draws, into textures its device can open, so
//! pictures reach the screen without a trip through system memory.

use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use ffmpeg_sys_next as sys;
use windows::core::Interface;
use windows::Win32::Foundation::{HANDLE, LUID};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_12_0;
use windows::Win32::Graphics::Direct3D12::{
    D3D12CreateDevice, ID3D12Device, ID3D12Fence, ID3D12Resource, D3D12_HEAP_FLAG_SHARED,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, IDXGIAdapter, IDXGIFactory4, DXGI_CREATE_FACTORY_FLAGS,
};

use super::hardware::DeviceRef;
use super::Picture;

/// Pictures one decoder keeps on the GPU at once; later ones are read back until some
/// are let go, so playback holding on to many can't run the decoder's pool dry.
pub(super) const MOST_HELD: usize = 16;

/// A d3d12va device on the adapter with `luid`, with frames in heaps other devices
/// can open.
pub(super) fn open_device(luid: [u8; 8]) -> Option<DeviceRef> {
    let [low @ .., _, _, _, _] = luid;
    let [_, _, _, _, high @ ..] = luid;
    let luid = LUID {
        LowPart: u32::from_le_bytes(low),
        HighPart: i32::from_le_bytes(high),
    };
    // SAFETY: plain calls whose failures come back as errors; the device is returned
    // with a reference of its own.
    let device: ID3D12Device = unsafe {
        let factory: IDXGIFactory4 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)).ok()?;
        let adapter: IDXGIAdapter = factory.EnumAdapterByLuid(luid).ok()?;
        let mut device = None;
        D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_12_0, &mut device).ok()?;
        device?
    };
    let kind = sys::AVHWDeviceType::AV_HWDEVICE_TYPE_D3D12VA;
    // SAFETY: a null return means allocation failed.
    let buffer = NonNull::new(unsafe { sys::av_hwdevice_ctx_alloc(kind) })?;
    let reference = DeviceRef::new(buffer);
    // SAFETY: the buffer holds a new d3d12va AVHWDeviceContext, whose hwctx is an
    // AVD3D12VADeviceContext to fill before init. The context takes over the device's
    // reference and releases it when freed, whether or not init succeeds.
    unsafe {
        let context = (*buffer.as_ptr()).data.cast::<sys::AVHWDeviceContext>();
        let hardware = (*context).hwctx.cast::<sys::AVD3D12VADeviceContext>();
        (*hardware).device = device.into_raw().cast();
        (*hardware).heap_flags = D3D12_HEAP_FLAG_SHARED.0 as sys::D3D12_HEAP_FLAGS;
        if sys::av_hwdevice_ctx_init(buffer.as_ptr()) < 0 {
            return None;
        }
    }
    Some(reference)
}

/// Counts the pictures a decoder has on the GPU.
#[derive(Default)]
pub(super) struct Holds(Arc<AtomicUsize>);

/// One picture kept on the GPU, counted until it drops.
pub(crate) struct Hold(Arc<AtomicUsize>);

impl Holds {
    /// Another hold, unless as many as allowed are out.
    pub(super) fn take(&self) -> Option<Hold> {
        let counted = self
            .0
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |held| {
                (held < MOST_HELD).then_some(held + 1)
            });
        counted.ok().map(|_| Hold(Arc::clone(&self.0)))
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// A decoded picture in a D3D12 texture of its own, which the decoder has finished
/// writing. NV12 for 8-bit video, P010 for deeper.
pub struct GpuSurface {
    texture: ID3D12Resource,
}

impl GpuSurface {
    /// The decoder's texture, in a shared heap, so another device can open it with
    /// `CreateSharedHandle`. It holds this picture until the frame it came with drops.
    pub fn texture(&self) -> &ID3D12Resource {
        &self.texture
    }

    /// The surface of a picture kept on the GPU, once decoded; `None` for pictures
    /// elsewhere or sharing a texture array, which can't be opened as one picture.
    pub(super) fn of(picture: &Picture) -> Option<Self> {
        let raw = picture.frame().raw();
        if raw.format != sys::AVPixelFormat::AV_PIX_FMT_D3D12 as i32 {
            return None;
        }
        let frame = raw.data[0].cast::<sys::AVD3D12VAFrame>();
        if frame.is_null() {
            return None;
        }
        let array = sys::AVD3D12VAFrameFlags::AV_D3D12VA_FRAME_FLAG_TEXTURE_ARRAY as i32;
        // SAFETY: a D3D12 frame's first data pointer is its AVD3D12VAFrame, alive while
        // the frame is. The flags are read as the integer FFmpeg stores, as they may
        // combine into a value the Rust enum has no variant for.
        let (texture, subresource, fence, value, flags) = unsafe {
            (
                (*frame).texture,
                (*frame).subresource_index,
                (*frame).sync_ctx.fence,
                (*frame).sync_ctx.fence_value,
                ptr::addr_of!((*frame).flags).cast::<i32>().read(),
            )
        };
        if flags & array != 0 || subresource != 0 {
            return None;
        }
        let texture = texture.cast::<std::ffi::c_void>();
        let fence = fence.cast::<std::ffi::c_void>();
        // SAFETY: the frame holds a reference to both while it lives; cloning the
        // texture takes one of our own.
        let (texture, fence) = unsafe {
            (
                ID3D12Resource::from_raw_borrowed(&texture)?.clone(),
                ID3D12Fence::from_raw_borrowed(&fence)?,
            )
        };
        // The decoder signals the fence once its GPU work on the picture is done. A
        // null event makes the call wait for that here, on the decoding thread.
        // SAFETY: the fence is alive for as long as the frame.
        unsafe {
            if fence.GetCompletedValue() < value {
                fence.SetEventOnCompletion(value, HANDLE::default()).ok()?;
            }
        }
        Some(Self { texture })
    }
}
