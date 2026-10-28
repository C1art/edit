use egui::{PointerButton, Rect, Vec2};

/// How big the program monitor shows the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum MonitorZoom {
    /// As large as fits the monitor.
    #[default]
    Fit,
    /// A sequence pixel to this many hundredths of a screen pixel.
    Percent(u16),
}

impl MonitorZoom {
    pub(crate) const ALL: [Self; 9] = [
        Self::Fit,
        Self::Percent(10),
        Self::Percent(25),
        Self::Percent(50),
        Self::Percent(75),
        Self::Percent(100),
        Self::Percent(150),
        Self::Percent(200),
        Self::Percent(400),
    ];

    pub(crate) fn name(self) -> String {
        match self {
            Self::Fit => "Fit".to_owned(),
            Self::Percent(percent) => format!("{percent}%"),
        }
    }
}

/// The program monitor's zoom, and how far a zoomed picture is moved from the middle.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ProgramZoom {
    zoom: MonitorZoom,
    pan: Vec2,
}

impl ProgramZoom {
    pub(crate) fn zoom(&self) -> MonitorZoom {
        self.zoom
    }

    pub(crate) fn set_zoom(&mut self, zoom: MonitorZoom) {
        *self = Self {
            zoom,
            pan: Vec2::ZERO,
        };
    }

    /// Where a `canvas`-sized picture goes in the monitor's `area`, moved by scrolling
    /// or middle-dragging over `response` when it is larger than the monitor.
    pub(crate) fn place(&mut self, ui: &egui::Ui, response: &egui::Response, canvas: Vec2) -> Rect {
        let area = response.rect;
        let size = match self.zoom {
            MonitorZoom::Fit => canvas * (area.width() / canvas.x).min(area.height() / canvas.y),
            // Percentages count physical pixels, as the sequence's own are.
            MonitorZoom::Percent(percent) => {
                canvas * f32::from(percent) / 100.0 / ui.ctx().pixels_per_point()
            }
        };
        if response.hovered() {
            self.pan += ui.input(|input| input.smooth_scroll_delta);
        }
        if response.dragged_by(PointerButton::Middle) {
            self.pan += response.drag_delta();
        }
        // Far enough to bring each edge of a picture larger than the monitor into view.
        let reach = ((size - area.size()) / 2.0).max(Vec2::ZERO);
        self.pan = self.pan.clamp(-reach, reach);
        Rect::from_center_size(area.center() + self.pan, size)
    }
}
