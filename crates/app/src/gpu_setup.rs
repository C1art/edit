use eframe::egui_wgpu;

/// egui's renderer setup, asking the device for what drawing pictures where they were
/// decoded needs, when the adapter has it.
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
    }
    config
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
