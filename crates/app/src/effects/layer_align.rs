use timeline::{GraphicLayer, Pin, VerticalPin};

/// Buttons that put a layer's edge or middle on the frame's, pinning that edge so the
/// layer stays aligned as its text grows; true when one was clicked.
pub(super) fn align_row(
    ui: &mut egui::Ui,
    layer: &mut GraphicLayer,
    [width, height]: [u32; 2],
) -> bool {
    let (half_width, half_height) = ((width / 2) as i32, (height / 2) as i32);
    let mut clicked = false;
    ui.horizontal_wrapped(|ui| {
        ui.label("Align");
        for (pin, x) in [
            (Pin::Left, -half_width),
            (Pin::Center, 0),
            (Pin::Right, half_width),
        ] {
            if ui
                .small_button(pin.name())
                .on_hover_text(format!("Puts the layer's {} on the frame's", pin_edge(pin)))
                .clicked()
            {
                (layer.pin, layer.offset[0]) = (pin, x);
                clicked = true;
            }
        }
        ui.separator();
        for (pin, y) in [
            (VerticalPin::Top, -half_height),
            (VerticalPin::Middle, 0),
            (VerticalPin::Bottom, half_height),
        ] {
            if ui
                .small_button(pin.name())
                .on_hover_text(format!(
                    "Puts the layer's {} on the frame's",
                    vertical_edge(pin)
                ))
                .clicked()
            {
                (layer.pin_vertical, layer.offset[1]) = (pin, y);
                clicked = true;
            }
        }
    });
    clicked
}

fn pin_edge(pin: Pin) -> &'static str {
    match pin {
        Pin::Left => "left edge",
        Pin::Center => "center",
        Pin::Right => "right edge",
    }
}

fn vertical_edge(pin: VerticalPin) -> &'static str {
    match pin {
        VerticalPin::Top => "top edge",
        VerticalPin::Middle => "middle",
        VerticalPin::Bottom => "bottom edge",
    }
}
