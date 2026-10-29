//! The editor's look: neutral dark greys that keep attention on the picture, one blue
//! accent for what is selected or active, and panels set apart by thin gutters.

use egui::epaint::Shadow;
use egui::{Color32, CornerRadius, FontFamily, FontId, Frame, Margin, Stroke, TextStyle, Theme};

/// Behind the panels, showing through the gaps between them.
pub(crate) const GUTTER: Color32 = Color32::from_rgb(17, 17, 19);
pub(crate) const PANEL: Color32 = Color32::from_rgb(30, 31, 34);
/// Wells inside a panel: monitors, track lanes, text fields.
pub(crate) const WELL: Color32 = Color32::from_rgb(21, 22, 24);
pub(crate) const RAISED: Color32 = Color32::from_rgb(43, 45, 50);
const HOVERED: Color32 = Color32::from_rgb(54, 57, 63);
const PRESSED: Color32 = Color32::from_rgb(66, 70, 77);
const LINE: Color32 = Color32::from_rgb(46, 48, 53);
pub(crate) const TEXT: Color32 = Color32::from_rgb(214, 216, 221);
pub(crate) const WEAK_TEXT: Color32 = Color32::from_rgb(138, 142, 151);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(64, 140, 255);
pub(crate) const ACCENT_FILL: Color32 = Color32::from_rgb(38, 98, 190);

const GAP: i8 = 3;
const PANEL_RADIUS: u8 = 6;

pub(crate) fn apply(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // The interface font lacks arrows, which the monospace font has.
    if let Some(proportional) = fonts.families.get_mut(&FontFamily::Proportional) {
        proportional.push("Hack".to_owned());
    }
    ctx.set_fonts(fonts);
    ctx.set_theme(Theme::Dark);
    ctx.style_mut_of(Theme::Dark, |style| {
        style.text_styles = [
            (TextStyle::Small, 10.5, FontFamily::Proportional),
            (TextStyle::Body, 13.0, FontFamily::Proportional),
            (TextStyle::Button, 13.0, FontFamily::Proportional),
            (TextStyle::Heading, 16.0, FontFamily::Proportional),
            (TextStyle::Monospace, 12.0, FontFamily::Monospace),
        ]
        .into_iter()
        .map(|(text, size, family)| (text, FontId::new(size, family)))
        .collect();

        let spacing = &mut style.spacing;
        spacing.item_spacing = egui::vec2(6.0, 5.0);
        spacing.button_padding = egui::vec2(8.0, 3.0);
        spacing.interact_size.y = 22.0;
        spacing.menu_margin = Margin::same(6);
        spacing.window_margin = Margin::same(12);
        spacing.indent = 16.0;
        spacing.scroll = egui::style::ScrollStyle::thin();

        let visuals = &mut style.visuals;
        visuals.override_text_color = None;
        visuals.panel_fill = GUTTER;
        visuals.window_fill = PANEL;
        visuals.window_stroke = Stroke::new(1.0, LINE);
        visuals.window_corner_radius = CornerRadius::same(8);
        visuals.menu_corner_radius = CornerRadius::same(6);
        visuals.window_shadow = Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(120),
        };
        visuals.popup_shadow = Shadow {
            offset: [0, 4],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(100),
        };
        visuals.extreme_bg_color = WELL;
        visuals.text_edit_bg_color = Some(WELL);
        visuals.faint_bg_color = Color32::from_rgb(35, 36, 40);
        visuals.code_bg_color = WELL;
        visuals.hyperlink_color = ACCENT;
        visuals.warn_fg_color = Color32::from_rgb(240, 170, 60);
        visuals.error_fg_color = Color32::from_rgb(240, 90, 90);
        visuals.selection.bg_fill = ACCENT_FILL;
        visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        visuals.striped = true;
        visuals.slider_trailing_fill = true;
        visuals.indent_has_left_vline = false;

        let radius = CornerRadius::same(4);
        let widgets = &mut visuals.widgets;
        widgets.noninteractive.bg_fill = PANEL;
        widgets.noninteractive.weak_bg_fill = PANEL;
        widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
        widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        widgets.noninteractive.corner_radius = radius;
        for (state, fill, text) in [
            (&mut widgets.inactive, RAISED, TEXT),
            (&mut widgets.hovered, HOVERED, Color32::WHITE),
            (&mut widgets.active, PRESSED, Color32::WHITE),
            (&mut widgets.open, HOVERED, Color32::WHITE),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = Stroke::NONE;
            state.fg_stroke = Stroke::new(1.0, text);
            state.corner_radius = radius;
            state.expansion = 0.0;
        }
        widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(78, 82, 90));
        widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    });
}

/// The frame of a docked panel: the gutter around a rounded card.
pub(crate) fn dock() -> Frame {
    Frame::new().fill(GUTTER).inner_margin(Margin::same(GAP))
}

/// Fills `ui` with a rounded panel card and lays `content` out inside it.
pub(crate) fn card<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    focus_card(ui, false, content)
}

/// A panel card outlined in the accent while it has the keyboard focus.
pub(crate) fn focus_card<R>(
    ui: &mut egui::Ui,
    focused: bool,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    // Always stroked, so that gaining focus doesn't shift the contents.
    let outline = if focused {
        ACCENT.gamma_multiply(0.8)
    } else {
        PANEL
    };
    Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, outline))
        .corner_radius(CornerRadius::same(PANEL_RADIUS))
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            // Content too wide is cut off rather than growing the panel: its size is
            // the workspace's, and would otherwise jump with what is selected.
            let room = ui.available_rect_before_wrap();
            let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(room));
            inner.set_clip_rect(room.intersect(ui.clip_rect()));
            let shown = content(&mut inner);
            ui.advance_cursor_after_rect(room);
            shown
        })
        .inner
}

/// A row of text tabs, the current one bright and underlined in the accent. Tabs that
/// don't fit go in a menu at the end of the row, never the current one.
pub(crate) fn tab_bar<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    current: &mut T,
    tabs: impl IntoIterator<Item = T>,
    title: impl Fn(T) -> &'static str,
) {
    const GAP: f32 = 16.0;
    const OVERFLOW_WIDTH: f32 = 24.0;
    let tabs: Vec<T> = tabs.into_iter().collect();
    let widths: Vec<f32> = tabs
        .iter()
        .map(|&tab| tab_galley(ui, title(tab)).size().x)
        .collect();
    let all = widths.iter().sum::<f32>() + GAP * widths.len().saturating_sub(1) as f32;
    let available = ui.available_width();
    let shown: Vec<bool> = if all <= available {
        vec![true; tabs.len()]
    } else {
        let selected = tabs.iter().position(|tab| tab == current);
        let mut room = available - OVERFLOW_WIDTH;
        if let Some(index) = selected {
            room -= widths[index] + GAP;
        }
        let mut fitting = true;
        (0..tabs.len())
            .map(|index| {
                if Some(index) == selected {
                    return true;
                }
                fitting &= widths[index] + GAP <= room;
                room -= widths[index] + GAP;
                fitting
            })
            .collect()
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(GAP, 6.0);
        for (&tab, _) in tabs.iter().zip(&shown).filter(|(_, &shown)| shown) {
            if self::tab(ui, *current == tab, title(tab)).clicked() {
                *current = tab;
            }
        }
        let hidden: Vec<T> = tabs
            .iter()
            .zip(&shown)
            .filter(|(_, &shown)| !shown)
            .map(|(&tab, _)| tab)
            .collect();
        if !hidden.is_empty() {
            ui.menu_button("»", |ui| {
                for tab in hidden {
                    if ui.button(title(tab)).clicked() {
                        *current = tab;
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text("More panels");
        }
    });
    ui.add_space(2.0);
}

fn tab_galley(ui: &egui::Ui, title: &str) -> std::sync::Arc<egui::Galley> {
    egui::WidgetText::from(egui::RichText::new(title).size(13.5)).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        TextStyle::Button,
    )
}

/// A tab's title, bright and underlined in the accent while selected.
pub(crate) fn tab(ui: &mut egui::Ui, selected: bool, title: &str) -> egui::Response {
    let galley = tab_galley(ui, title);
    let (rect, response) = ui.allocate_exact_size(galley.size(), egui::Sense::click());
    let color = match (selected, response.hovered()) {
        (true, _) => Color32::WHITE,
        (false, true) => TEXT,
        (false, false) => WEAK_TEXT,
    };
    ui.painter().galley(rect.min, galley, color);
    if selected {
        let underline = rect.bottom() + 3.0;
        ui.painter()
            .hline(rect.x_range(), underline, Stroke::new(2.0, ACCENT));
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Lays `content` out as one segmented control: its selectable labels sit side by side
/// in a shared well.
pub(crate) fn segmented<R>(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui) -> R) -> R {
    // A wrapping row only breaks between widgets, so a group that won't fit what is
    // left of the row, going by its width last frame, starts the next row itself.
    let width_id = ui.next_auto_id().with("segmented width");
    let width: Option<f32> = ui.data(|data| data.get_temp(width_id));
    let row_started = ui.cursor().left() > ui.max_rect().left();
    let room = ui.available_size_before_wrap().x;
    if ui.layout().main_wrap && row_started && width.is_some_and(|width| width > room) {
        ui.end_row();
    }
    let shown = Frame::new()
        .fill(WELL)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().button_padding.x = 6.0;
            ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
            ui.horizontal(content).inner
        });
    let width = shown.response.rect.width();
    ui.data_mut(|data| data.insert_temp(width_id, width));
    shown.inner
}
