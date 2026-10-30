use crate::editor::Editor;

/// The choices kept from one session to the next.
pub(crate) fn menu(editor: &mut Editor, ui: &mut egui::Ui) {
    let decoding = &mut editor.hardware_decoding;
    let device = media::hardware_decoder().map_or_else(
        || "No GPU decoder is in use yet.".to_owned(),
        |name| format!("Decoding on {name}."),
    );
    let hint = format!("Applies to media opened from now on. {device}");
    if ui
        .checkbox(decoding, "Hardware-Accelerated Decoding")
        .on_hover_text(hint)
        .changed()
    {
        media::set_hardware_decoding(*decoding);
    }
    ui.horizontal(|ui| {
        let autosave = &mut editor.autosave;
        ui.checkbox(&mut autosave.enabled, "Auto-Save Every");
        ui.add_enabled(
            autosave.enabled,
            egui::DragValue::new(&mut autosave.minutes)
                .range(1..=60)
                .suffix(" min"),
        );
    });
    ui.horizontal(|ui| {
        ui.label("Default Transition Duration");
        let length = &mut editor.transition_defaults.length;
        ui.add(hundredths_field(
            length,
            crate::transition_defaults::TransitionDefaults::LENGTHS,
        ))
        .on_hover_text(
            "Right-click a video transition in the Effects panel to make it the default",
        );
    });
    ui.horizontal(|ui| {
        ui.label("Still Image Default Duration");
        ui.add(hundredths_field(
            &mut editor.still_length.0,
            crate::still_length::StillLength::LENGTHS,
        ))
        .on_hover_text("How long stills and titles brought in from now on place for");
    });
    ui.menu_button("Default Media Scaling", |ui| {
        let choice = &mut editor.media_scaling;
        for option in crate::media_scaling::MediaScaling::ALL {
            ui.radio_value(choice, option, option.label());
        }
        ui.weak("For media brought in from now on");
    });
    ui.menu_button("Timeline Playback Auto-Scrolling", |ui| {
        let choice = &mut editor.view.auto_scroll;
        for option in crate::timeline_view::AutoScroll::ALL {
            ui.radio_value(choice, option, option.label());
        }
    });
    ui.menu_button("Proxies for 4K and High Frame Rate Media", |ui| {
        let choice = &mut editor.proxy_offer.choice;
        for option in crate::proxy_offer::HeavyMedia::ALL {
            ui.radio_value(choice, option, option.label());
        }
    });
    ui.menu_button("Audio Output", |ui| {
        crate::audio_switch::menu(editor, ui);
    });
}

/// Hundredths of a second shown and typed as seconds.
fn hundredths_field(
    hundredths: &mut u32,
    range: std::ops::RangeInclusive<u32>,
) -> egui::DragValue<'_> {
    egui::DragValue::new(hundredths)
        .range(range)
        .custom_formatter(|hundredths, _| format!("{:.2} s", hundredths / 100.0))
        .custom_parser(|text| {
            let seconds = text.trim().trim_end_matches('s').trim();
            seconds.parse::<f64>().ok().map(|seconds| seconds * 100.0)
        })
}
