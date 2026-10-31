use egui::{Context, Pos2, Vec2, ViewportBuilder};

use crate::editor::Editor;
use crate::project_store::Setting;

/// Below this the panels no longer fit side by side.
pub(crate) const SMALLEST: Vec2 = Vec2::new(960.0, 540.0);

/// The window's size and place when not maximized, and whether it was, so it opens as
/// it was left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowState {
    size: Vec2,
    maximized: bool,
    placement: Option<Placement>,
}

/// Where the window's frame had its top left corner, in points, and how many pixels a
/// point was there, to find the spot again among the monitors.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Placement {
    corner: Pos2,
    pixels_per_point: f32,
}

impl Placement {
    /// Whether the title bar would land on a monitor connected now, as Windows leaves a
    /// window wherever it is put, even off every screen. Elsewhere the system places
    /// windows itself.
    fn is_on_screen(self) -> bool {
        #[cfg(windows)]
        {
            let title_bar = (self.corner + Vec2::new(100.0, 10.0)) * self.pixels_per_point;
            on_screen::has_monitor_at(title_bar)
        }
        #[cfg(not(windows))]
        false
    }
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            size: Vec2::new(1600.0, 900.0),
            maximized: false,
            placement: None,
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

    /// Reads "width height", then "maximized" if it was, then "at x y pixels-per-point".
    fn from_text(text: &str) -> Option<Self> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let number = |index: usize| {
            let value = words.get(index)?.parse::<f32>().ok();
            value.filter(|value| value.is_finite())
        };
        let size = Vec2::new(number(0)?, number(1)?).max(SMALLEST);
        let maximized = words.get(2) == Some(&"maximized");
        let placement = words.iter().position(|&word| word == "at").and_then(|at| {
            Some(Placement {
                corner: Pos2::new(number(at + 1)?, number(at + 2)?),
                pixels_per_point: number(at + 3).filter(|&scale| scale > 0.0)?,
            })
        });
        Some(Self {
            size,
            maximized,
            placement,
        })
    }

    fn to_text(self) -> String {
        let maximized = if self.maximized { " maximized" } else { "" };
        let placement = self.placement.map_or_else(String::new, |placement| {
            let Placement {
                corner,
                pixels_per_point,
            } = placement;
            format!(
                " at {} {} {pixels_per_point}",
                corner.x.round(),
                corner.y.round()
            )
        });
        format!(
            "{} {}{maximized}{placement}\n",
            self.size.x.round(),
            self.size.y.round()
        )
    }

    pub fn open(self, builder: ViewportBuilder) -> ViewportBuilder {
        // eframe shrinks a size too big for every monitor, so one saved on a larger
        // screen still fits.
        let builder = builder
            .with_inner_size(self.size)
            .with_min_inner_size(SMALLEST)
            .with_maximized(self.maximized);
        match self.placement_on_screen() {
            Some(placement) => builder.with_position(placement.corner),
            None => builder,
        }
    }

    /// Whether the window opens in the middle of the screen, having no place to go back to.
    pub fn centered(self) -> bool {
        self.placement_on_screen().is_none()
    }

    fn placement_on_screen(self) -> Option<Placement> {
        self.placement.filter(|placement| placement.is_on_screen())
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
            if self.window.maximized {
                return;
            }
            if let Some(inner) = viewport.inner_rect {
                self.window.size = inner.size();
            }
            if let (Some(outer), Some(pixels_per_point)) =
                (viewport.outer_rect, viewport.native_pixels_per_point)
            {
                self.window.placement = Some(Placement {
                    corner: outer.min,
                    pixels_per_point,
                });
            }
        });
        if closes {
            if let Some(store) = &mut self.store {
                store.write_setting(Setting::Window, self.window.to_text());
            }
        }
    }
}

#[cfg(windows)]
mod on_screen {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONULL};

    /// `pixel` is in the desktop's pixels, across every monitor.
    pub(super) fn has_monitor_at(pixel: egui::Pos2) -> bool {
        let point = POINT {
            x: pixel.x as i32,
            y: pixel.y as i32,
        };
        // SAFETY: MonitorFromPoint only reads the point passed by value and, with
        // MONITOR_DEFAULTTONULL, returns null when no monitor holds it; the handle it
        // returns needs no release.
        !unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONULL) }
            .0
            .is_null()
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
            placement: Some(Placement {
                corner: Pos2::new(-1700.0, 40.0),
                pixels_per_point: 1.25,
            }),
        };
        assert_eq!(WindowState::from_text(&left.to_text()), Some(left));
        let unplaced = WindowState::from_text("1280 720 maximized");
        assert_eq!(unplaced.map(|state| state.placement), Some(None));
        let tiny = WindowState::from_text("200 100").map(|state| state.size);
        assert_eq!(tiny, Some(SMALLEST));
        assert_eq!(WindowState::from_text("wide 720"), None);
    }
}
