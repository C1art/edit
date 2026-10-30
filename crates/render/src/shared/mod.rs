//! Pictures decoded with D3D12, opened as textures where they were decoded instead of
//! being copied through system memory.

mod dx12;

use timeline::{PlaneFormat, SampleDepth};
use windows::core::Interface;
use windows::Win32::Foundation::{CloseHandle, GENERIC_ALL, HANDLE};
use windows::Win32::Graphics::Direct3D12::{ID3D12Device, ID3D12Resource};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_NV12, DXGI_FORMAT_P010};

/// Opened textures left unused by this many composites are let go, as each keeps a
/// picture of a decoder's pool, maybe of a file since closed, alive.
const UNUSED_COMPOSITES: u64 = 120;

/// What a DX12 device needs to open decoded textures: their formats and views of
/// their planes. Vulkan isn't asked, as it lays the planes of an imported NV12 texture
/// out differently from D3D12, reading the second where there's nothing and past the
/// end of the memory, which loses the device.
fn features(backend: wgpu::Backend) -> wgpu::Features {
    match backend {
        wgpu::Backend::Dx12 => {
            wgpu::Features::TEXTURE_FORMAT_NV12
                | wgpu::Features::TEXTURE_FORMAT_P010
                | wgpu::Features::TEXTURE_FORMAT_16BIT_NORM
        }
        _ => wgpu::Features::empty(),
    }
}

/// The features to ask a device on `adapter` for, so it can draw pictures decoded on
/// it without copying them: none unless the adapter has them all.
pub fn texture_sharing_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    let wanted = features(adapter.get_info().backend);
    if !wanted.is_empty() && adapter.features().contains(wanted) {
        wanted
    } else {
        wgpu::Features::empty()
    }
}

/// The LUID of the adapter to decode on so the compositor draws the decoder's textures
/// directly, when the device was made with [`texture_sharing_features`] on DX12.
pub fn decode_adapter(device: &wgpu::Device) -> Option<[u8; 8]> {
    let wanted = features(device.adapter_info().backend);
    if wanted.is_empty() || !device.features().contains(wanted) {
        return None;
    }
    dx12::adapter_luid(device)
}

/// A decoder's texture opened on the compositor's device, with views of its planes.
pub(crate) struct Opened {
    /// Holds the decoder's texture, so its address names it while this lives.
    resource: ID3D12Resource,
    pub(crate) luma: wgpu::TextureView,
    pub(crate) chroma: wgpu::TextureView,
    used: u64,
}

/// Decoders reuse a pool of textures, so each is opened once and found by address.
#[derive(Default)]
pub(crate) struct SharedTextures {
    opened: Vec<Opened>,
    composites: u64,
}

impl SharedTextures {
    pub(crate) fn open(
        &mut self,
        device: &wgpu::Device,
        resource: &ID3D12Resource,
        format: PlaneFormat,
    ) -> Option<&Opened> {
        let index = match self
            .opened
            .iter()
            .position(|opened| opened.resource.as_raw() == resource.as_raw())
        {
            Some(index) => index,
            None => {
                let (luma, chroma) = open(device, resource, format)?;
                self.opened.push(Opened {
                    resource: resource.clone(),
                    luma,
                    chroma,
                    used: 0,
                });
                self.opened.len() - 1
            }
        };
        let opened = &mut self.opened[index];
        opened.used = self.composites;
        Some(opened)
    }

    pub(crate) fn finish_composite(&mut self) {
        self.composites += 1;
        let composites = self.composites;
        self.opened
            .retain(|opened| composites - opened.used <= UNUSED_COMPOSITES);
    }
}

/// Views of the planes of a decoder's texture, opened on `device`.
fn open(
    device: &wgpu::Device,
    resource: &ID3D12Resource,
    format: PlaneFormat,
) -> Option<(wgpu::TextureView, wgpu::TextureView)> {
    let (texture_format, dxgi_format, luma_format, chroma_format) = match format.depth {
        SampleDepth::Eight => (
            wgpu::TextureFormat::NV12,
            DXGI_FORMAT_NV12,
            wgpu::TextureFormat::R8Unorm,
            wgpu::TextureFormat::Rg8Unorm,
        ),
        SampleDepth::Sixteen => (
            wgpu::TextureFormat::P010,
            DXGI_FORMAT_P010,
            wgpu::TextureFormat::R16Unorm,
            wgpu::TextureFormat::Rg16Unorm,
        ),
        SampleDepth::Ten | SampleDepth::Twelve => return None,
    };
    // SAFETY: only describes the resource.
    let description = unsafe { resource.GetDesc() };
    if description.Format != dxgi_format
        || description.DepthOrArraySize != 1
        || description.MipLevels != 1
    {
        return None;
    }
    let descriptor = wgpu::TextureDescriptor {
        label: Some("decoded"),
        // Decoders pad pictures to whole blocks, so the texture can be larger.
        size: wgpu::Extent3d {
            width: u32::try_from(description.Width).ok()?,
            height: description.Height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: texture_format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    };
    let texture = dx12::open(device, resource, &descriptor)?;
    let view = |aspect, format| {
        texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("decoded plane"),
            format: Some(format),
            aspect,
            ..Default::default()
        })
    };
    Some((
        view(wgpu::TextureAspect::Plane0, luma_format),
        view(wgpu::TextureAspect::Plane1, chroma_format),
    ))
}

/// The device a resource was made on.
fn device_of(resource: &ID3D12Resource) -> Option<ID3D12Device> {
    let mut device = None;
    // SAFETY: GetDevice writes a new reference to the device into `device`.
    unsafe { resource.GetDevice(&mut device) }.ok()?;
    device
}

/// An NT handle to a resource in a shared heap, which other devices and APIs open.
struct SharedHandle(HANDLE);

impl SharedHandle {
    fn new(resource: &ID3D12Resource) -> Option<Self> {
        let device = device_of(resource)?;
        // SAFETY: the resource is alive and of `device`; the handle returned is ours to
        // close.
        let handle =
            unsafe { device.CreateSharedHandle(resource, None, GENERIC_ALL.0, None) }.ok()?;
        Some(Self(handle))
    }
}

impl Drop for SharedHandle {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateSharedHandle and is closed once. Opening
        // it took references of its own, so closing loses nothing.
        let _ = unsafe { CloseHandle(self.0) };
    }
}
