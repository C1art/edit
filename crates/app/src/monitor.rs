use std::time::Duration;

use eframe::egui_wgpu::{self, wgpu};
use egui::{Color32, Rect, Response, Sense, TextureId, Vec2};
use playback::ComposedFrame;
use render::{Background, Compositor, LayerImage, Presented, RenderError, Scope};

/// A video monitor: composites frames on the GPU and shows the result in the UI.
pub struct Monitor {
    render_state: egui_wgpu::RenderState,
    compositor: Compositor,
    texture: Option<TextureId>,
    scope: Option<TextureId>,
    /// The last frame asked for, while a shader it needs is still being built.
    pending: Option<(ComposedFrame, (u32, u32))>,
    /// Whether the picture's transparent parts show a checkerboard rather than black.
    transparency_grid: bool,
}

/// How often a monitor looks again for a shader being built.
const COMPILE_POLL: Duration = Duration::from_millis(15);

impl Monitor {
    pub fn new(render_state: egui_wgpu::RenderState) -> Self {
        let mut compositor = Compositor::new(&render_state.device, &render_state.queue);
        compositor.compile_in_background();
        Self {
            compositor,
            render_state,
            texture: None,
            scope: None,
            pending: None,
            transparency_grid: false,
        }
    }

    /// Shows `frame`, or keeps the last one shown until the shaders `frame` needs are
    /// built; [`Monitor::catch_up`] then shows it.
    pub fn present(
        &mut self,
        frame: &ComposedFrame,
        canvas: (u32, u32),
    ) -> Result<(), RenderError> {
        self.pending = None;
        let layers: Vec<LayerImage<'_>> = frame.layers.iter().map(playback::Layer::image).collect();
        let presented = self.compositor.composite(canvas, &layers)?;
        if presented == Presented::Pending {
            self.pending = Some((frame.clone(), canvas));
            return Ok(());
        }
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

    /// Shows a frame left waiting for its shaders once they're built; call every UI
    /// frame.
    pub fn catch_up(&mut self, ctx: &egui::Context) -> Result<(), RenderError> {
        if self.pending.is_none() {
            return Ok(());
        }
        if self.compositor.compiling() {
            ctx.request_repaint_after(COMPILE_POLL);
            return Ok(());
        }
        let Some((frame, canvas)) = self.pending.take() else {
            return Ok(());
        };
        self.present(&frame, canvas)?;
        ctx.request_repaint();
        Ok(())
    }

    pub fn set_scope(&mut self, scope: Option<Scope>) {
        self.compositor.set_scope(scope);
        if scope.is_some() == self.scope.is_some() {
            return;
        }
        let mut renderer = self.render_state.renderer.write();
        match (self.scope.take(), self.compositor.scope_view()) {
            (Some(id), _) => renderer.free_texture(&id),
            (None, Some(view)) => {
                let device = &self.render_state.device;
                self.scope =
                    Some(renderer.register_native_texture(device, view, wgpu::FilterMode::Linear));
            }
            (None, None) => {}
        }
    }

    pub fn scope(&self) -> Option<TextureId> {
        self.scope
    }

    /// Returns whether it changed, and so whether the frame needs presenting again.
    pub fn set_transparency_grid(&mut self, shown: bool) -> bool {
        if self.transparency_grid == shown {
            return false;
        }
        self.transparency_grid = shown;
        self.compositor.set_background(if shown {
            Background::UnderUi
        } else {
            Background::Black
        });
        true
    }

    /// Draws the picture letterboxed to `size` on a black background.
    pub fn ui(&self, ui: &mut egui::Ui, size: Vec2) -> Response {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
        self.paint(ui.painter(), rect);
        response
    }

    /// Draws the picture letterboxed into `rect`.
    pub fn paint(&self, painter: &egui::Painter, rect: Rect) {
        if let (Some(texture), Some((width, height))) =
            (self.texture, self.compositor.output_size())
        {
            let frame = Vec2::new(width as f32, height as f32);
            let scale = (rect.width() / frame.x).min(rect.height() / frame.y);
            let shown = Rect::from_center_size(rect.center(), frame * scale);
            if self.transparency_grid {
                crate::transparency_grid::paint(painter, shown);
            }
            let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.image(texture, shown, uv, Color32::WHITE);
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        let mut renderer = self.render_state.renderer.write();
        for id in [self.texture, self.scope].into_iter().flatten() {
            renderer.free_texture(&id);
        }
    }
}
