use egui::{Context, Vec2, ViewportBuilder};

use crate::editor::Editor;
use crate::project_store::Setting;

/// Below this the panels no longer fit side by side.
const SMALLEST: Vec2 = Vec2::new(960.0, 540.0);

/// The window's size when not maximized, and whether it was, so it opens as it was left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowState {
    size: Vec2,
    maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            size: Vec2::new(1600.0, 900.0),
            maximized: false,
        }
    }
}

impl WindowState {
    /// Read straight from disk, as how the window opens has to be known before it does.
    pub fn load() -> Self {
        std::fs::read_to_string(Setting::Window.path())
            .ok()
            .and_then(|text| Self::from_text(&text))
            .unwrap_or_default()
    }

    fn from_text(text: &str) -> Option<Self> {
        let mut words = text.split_whitespace();
        let mut length = || words.next()?.parse::<f32>().ok().filter(|v| v.is_finite());
        let size = Vec2::new(length()?, length()?).max(SMALLEST);
        let maximized = text.split_whitespace().nth(2) == Some("maximized");
        Some(Self { size, maximized })
    }

    fn to_text(self) -> String {
        let maximized = if self.maximized { " maximized" } else { "" };
        format!(
            "{} {}{maximized}\n",
            self.size.x.round(),
            self.size.y.round()
        )
    }

    pub fn open(self, builder: ViewportBuilder) -> ViewportBuilder {
        // eframe shrinks a size too big for every monitor, so one saved on a larger
        // screen still fits.
        builder
            .with_inner_size(self.size)
            .with_min_inner_size(SMALLEST)
            .with_maximized(self.maximized)
    }
}

impl Editor {
    /// Follows how the window is left, writing it down as it closes.
    pub(crate) fn track_window(&mut self, ctx: &Context, closes: bool) {
        ctx.input(|input| {
            let viewport = input.viewport();
            // Minimizing or going full screen says nothing about how to open next time.
            if viewport.minimized == Some(true) || viewport.fullscreen == Some(true) {
                return;
            }
            self.window.maximized = viewport.maximized.unwrap_or(false);
            if let Some(inner) = viewport.inner_rect.filter(|_| !self.window.maximized) {
                self.window.size = inner.size();
            }
        });
        if closes {
            if let Some(store) = &mut self.store {
                store.write_setting(Setting::Window, self.window.to_text());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_reopens_as_left_and_never_smaller_than_the_panels_need() {
        let left = WindowState {
            size: Vec2::new(1280.0, 720.0),
            maximized: true,
        };
        assert_eq!(WindowState::from_text(&left.to_text()), Some(left));
        let tiny = WindowState::from_text("200 100").map(|state| state.size);
        assert_eq!(tiny, Some(SMALLEST));
        assert_eq!(WindowState::from_text("wide 720"), None);
    }
}
