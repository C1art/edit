use egui::{DragValue, Key};
use timeline::{ChannelMap, SetChannelMap, SourceId};

use crate::editor::Editor;

/// Picks which of a file's channels play on each side, as Premiere's Modify > Audio
/// Channels does for a camera that records a separate microphone on each channel.
pub(crate) struct ChannelsDialog {
    source: SourceId,
    name: String,
    pick: bool,
    /// Counted from one, as the dialog shows them.
    left: u16,
    right: u16,
}

/// The most channels a file is assumed to carry.
const MAX_CHANNELS: u16 = 32;

impl Editor {
    pub(crate) fn open_audio_channels(&mut self, source: SourceId) {
        let Some(item) = self.timeline.project().media_item(source) else {
            return;
        };
        if !(item.has_audio && item.is_file()) {
            self.notice("Only files with sound have channels to pick");
            return;
        }
        let map = item.channel_map.unwrap_or(ChannelMap { left: 0, right: 1 });
        self.channels_dialog = Some(ChannelsDialog {
            source,
            name: item.name.clone(),
            pick: item.channel_map.is_some(),
            left: map.left + 1,
            right: map.right + 1,
        });
    }
}

pub(crate) fn dialog(editor: &mut Editor, ctx: &egui::Context) {
    let Some(dialog) = &mut editor.channels_dialog else {
        return;
    };
    let mut done = false;
    let mut cancelled = false;
    crate::dialog::window(ctx, "Modify Audio Channels")
        .show(ctx, |ui| {
            ui.strong(&dialog.name);
            ui.separator();
            ui.radio_value(&mut dialog.pick, false, "Mix all channels to stereo");
            ui.radio_value(&mut dialog.pick, true, "Pick a source channel for each side:");
            ui.add_enabled_ui(dialog.pick, |ui| {
                for (side, channel) in [("Left", &mut dialog.left), ("Right", &mut dialog.right)] {
                    ui.horizontal(|ui| {
                        ui.label(side);
                        ui.add(DragValue::new(channel).range(1..=MAX_CHANNELS).prefix("Channel "));
                    });
                }
                ui.weak("The same channel on both sides plays it as mono. A channel the file lacks is silent.");
            });
            ui.separator();
            crate::dialog::footer(ui, |ui| {
                cancelled = ui.button("Cancel").clicked()
                    || ui.input(|input| input.key_pressed(Key::Escape));
                done = ui.add(crate::dialog::primary("OK")).clicked() || ui.input(|input| input.key_pressed(Key::Enter));
            });
        });
    if !(done || cancelled) {
        return;
    }
    let Some(dialog) = editor.channels_dialog.take() else {
        return;
    };
    if done {
        let map = dialog.pick.then(|| ChannelMap {
            left: dialog.left.saturating_sub(1),
            right: dialog.right.saturating_sub(1),
        });
        editor.apply_to_pool(SetChannelMap {
            source: dialog.source,
            map,
        });
    }
}
