use wgpu::hal::api::Dx12;
use windows::Win32::Graphics::Direct3D12::ID3D12Resource;

use super::{device_of, SharedHandle};

pub(super) fn adapter_luid(device: &wgpu::Device) -> Option<[u8; 8]> {
    // SAFETY: the raw device is only asked for its adapter, while `device` is borrowed.
    let hal = unsafe { device.as_hal::<Dx12>() }?;
    // SAFETY: as above.
    let luid = unsafe { hal.raw_device().GetAdapterLuid() };
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&luid.LowPart.to_le_bytes());
    bytes[4..].copy_from_slice(&luid.HighPart.to_le_bytes());
    Some(bytes)
}

/// The decoder's texture on a DX12 `device`, read straight from where it was decoded.
pub(super) fn open(
    device: &wgpu::Device,
    resource: &ID3D12Resource,
    descriptor: &wgpu::TextureDescriptor<'_>,
) -> Option<wgpu::Texture> {
    let resource = {
        // SAFETY: the raw device only opens the resource, while `device` is borrowed.
        let hal = unsafe { device.as_hal::<Dx12>() }?;
        let ours = hal.raw_device();
        // A D3D12 device is one per adapter, so decoding on ours usually shares it.
        if device_of(resource).as_ref() == Some(ours) {
            resource.clone()
        } else {
            let handle = SharedHandle::new(resource)?;
            let mut opened = None;
            // SAFETY: the handle names a live resource on the same adapter and stays open
            // for the call.
            unsafe { ours.OpenSharedHandle(handle.0, &mut opened) }.ok()?;
            opened?
        }
    };
    // SAFETY: the resource is a texture of the descriptor's size and format, with one
    // mip level and one sample, on the device's adapter.
    let texture = unsafe {
        wgpu::hal::dx12::Device::texture_from_raw(
            resource,
            descriptor.format,
            descriptor.dimension,
            descriptor.size,
            1,
            1,
        )
    };
    // Said to be ready to read so wgpu never changes its state with a barrier: reading
    // it promotes it from the common state the decoder leaves it in, and it falls back
    // there once the work is done, as the decoder expects to find it.
    // SAFETY: the texture was created from a raw DX12 resource on this device, matching
    // the descriptor, and its pixels are all written by the decoder.
    Some(unsafe {
        device.create_texture_from_hal::<Dx12>(texture, descriptor, wgpu::TextureUses::RESOURCE)
    })
}
