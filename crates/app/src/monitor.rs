use eframe::egui_wgpu::{self, wgpu};
use egui::{Color32, Rect, Sense, TextureId, Vec2};
use media::VideoFrame;
use render::{Compositor, Presented, RenderError};

/// The program monitor: composites frames on the GPU and shows the result in the UI.
pub struct Monitor {
    render_state: egui_wgpu::RenderState,
    compositor: Compositor,
    texture: Option<TextureId>,
}

impl Monitor {
    pub fn new(render_state: egui_wgpu::RenderState) -> Self {
        Self {
            compositor: Compositor::new(&render_state.device, &render_state.queue),
            render_state,
            texture: None,
        }
    }

    pub fn present(&mut self, frame: &VideoFrame) -> Result<(), RenderError> {
        let presented = self
            .compositor
            .present_frame(frame.width, frame.height, &frame.pixels)?;
        let (Presented::NewTarget, Some(view)) = (presented, self.compositor.output_view()) else {
            return Ok(());
        };
        let device = &self.render_state.device;
        let mut renderer = self.render_state.renderer.write();
        match self.texture {
            Some(id) => renderer.update_egui_texture_from_wgpu_texture(
                device,
                view,
                wgpu::FilterMode::Linear,
                id,
            ),
            None => {
                self.texture =
                    Some(renderer.register_native_texture(device, view, wgpu::FilterMode::Linear));
            }
        }
        Ok(())
    }

    /// Draws the frame letterboxed to the available space on a black background.
    pub fn ui(&self, ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
        let (Some(texture), Some((width, height))) = (self.texture, self.compositor.output_size())
        else {
            return;
        };
        let frame = Vec2::new(width as f32, height as f32);
        let scale = (rect.width() / frame.x).min(rect.height() / frame.y);
        let shown = Rect::from_center_size(rect.center(), frame * scale);
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(texture, shown, uv, Color32::WHITE);
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        if let Some(id) = self.texture {
            self.render_state.renderer.write().free_texture(&id);
        }
    }
}
