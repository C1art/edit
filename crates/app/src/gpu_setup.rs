use eframe::egui_wgpu;
#[cfg(windows)]
use eframe::egui_wgpu::wgpu;

/// egui's renderer setup on DX12 where it can, asking the device for what drawing
/// pictures where they were decoded needs, when the adapter has it.
pub(crate) fn wgpu_configuration() -> egui_wgpu::WgpuConfiguration {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut config = egui_wgpu::WgpuConfiguration::default();
    #[cfg(windows)]
    if let egui_wgpu::WgpuSetup::CreateNew(setup) = &mut config.wgpu_setup {
        let base = std::sync::Arc::clone(&setup.device_descriptor);
        setup.device_descriptor = std::sync::Arc::new(move |adapter| {
            let mut descriptor = base(adapter);
            descriptor.required_features |= render::texture_sharing_features(adapter);
            descriptor
        });
        setup.native_adapter_selector = Some(std::sync::Arc::new(choose_adapter));
    }
    config
}

/// The fastest kind of GPU that can draw to the window, on DX12 when it has it. wgpu on
/// its own takes the first backend it lists, Vulkan, where decoded pictures can't be
/// drawn without a copy through system memory. `WGPU_BACKEND` still narrows the list.
#[cfg(windows)]
fn choose_adapter(
    adapters: &[wgpu::Adapter],
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter, String> {
    let kind_rank = |kind| match kind {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Other => 3,
        wgpu::DeviceType::Cpu => 4,
    };
    let backend_rank = |backend| match backend {
        wgpu::Backend::Dx12 => 0,
        wgpu::Backend::Vulkan => 1,
        _ => 2,
    };
    adapters
        .iter()
        .filter(|adapter| surface.is_none_or(|surface| adapter.is_surface_supported(surface)))
        .min_by_key(|adapter| {
            let info = adapter.get_info();
            (kind_rank(info.device_type), backend_rank(info.backend))
        })
        .cloned()
        .ok_or_else(|| "no GPU can draw to the window".to_owned())
}

/// Has hardware decoders use the adapter `render_state` draws with, when the compositor
/// can open their textures there.
pub(crate) fn decode_where_drawn(render_state: Option<&egui_wgpu::RenderState>) {
    #[cfg(windows)]
    if let Some(luid) = render_state.and_then(|state| render::decode_adapter(&state.device)) {
        media::set_decode_adapter(luid);
    }
    #[cfg(not(windows))]
    let _ = render_state;
}
