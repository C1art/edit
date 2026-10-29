use crate::editor::Editor;
use crate::project_store::Setting;

/// The choices kept from one session to the next, written as `name<TAB>value` lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Preferences {
    hardware_decoding: bool,
    autosave: bool,
    autosave_minutes: u32,
    scrub_audio: bool,
    dropped_frame_indicator: bool,
    /// `None` follows the system default.
    audio_device: Option<String>,
}

impl Preferences {
    fn of(editor: &Editor) -> Self {
        Self {
            hardware_decoding: editor.hardware_decoding,
            autosave: editor.autosave.enabled,
            autosave_minutes: editor.autosave.minutes,
            scrub_audio: editor.scrub_audio,
            dropped_frame_indicator: editor.dropped_frame_indicator,
            audio_device: editor.audio_switch.chosen.clone(),
        }
    }

    pub(crate) fn to_text(&self) -> String {
        format!(
            "hardware_decoding\t{}\nautosave\t{}\nautosave_minutes\t{}\nscrub_audio\t{}\n\
             dropped_frame_indicator\t{}\naudio_device\t{}\n",
            self.hardware_decoding,
            self.autosave,
            self.autosave_minutes,
            self.scrub_audio,
            self.dropped_frame_indicator,
            self.audio_device.as_deref().unwrap_or_default(),
        )
    }

    /// Reads what `text` holds over `self`; lines that can't be read leave the choice
    /// as it was, so a file from another version still loads.
    pub(crate) fn read(mut self, text: &str) -> Self {
        for (name, value) in text.lines().filter_map(|line| line.split_once('\t')) {
            let flag = value.parse::<bool>().ok();
            match name {
                "hardware_decoding" => set(&mut self.hardware_decoding, flag),
                "autosave" => set(&mut self.autosave, flag),
                "autosave_minutes" => set(&mut self.autosave_minutes, value.parse().ok()),
                "scrub_audio" => set(&mut self.scrub_audio, flag),
                "dropped_frame_indicator" => set(&mut self.dropped_frame_indicator, flag),
                "audio_device" => {
                    self.audio_device = Some(value.to_owned()).filter(|name| !name.is_empty());
                }
                _ => {}
            }
        }
        self
    }
}

fn set<T>(choice: &mut T, read: Option<T>) {
    if let Some(read) = read {
        *choice = read;
    }
}

impl Editor {
    pub(crate) fn load_preferences(&mut self, text: &str) {
        let preferences = Preferences::of(self).read(text);
        self.hardware_decoding = preferences.hardware_decoding;
        media::set_hardware_decoding(preferences.hardware_decoding);
        self.autosave.enabled = preferences.autosave;
        self.autosave.minutes = preferences.autosave_minutes.clamp(1, 60);
        self.scrub_audio = preferences.scrub_audio;
        self.dropped_frame_indicator = preferences.dropped_frame_indicator;
        self.audio_switch.chosen = preferences.audio_device;
        self.saved_preferences = Some(Preferences::of(self));
    }

    /// Writes the preferences out whenever one changes, wherever it was changed from.
    pub(crate) fn keep_preferences(&mut self) {
        let preferences = Preferences::of(self);
        let saved = self.saved_preferences.replace(preferences.clone());
        if saved.is_none_or(|saved| saved == preferences) {
            return;
        }
        if let Some(store) = &mut self.store {
            store.write_setting(Setting::Preferences, preferences.to_text());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULTS: Preferences = Preferences {
        hardware_decoding: true,
        autosave: true,
        autosave_minutes: 5,
        scrub_audio: true,
        dropped_frame_indicator: false,
        audio_device: None,
    };

    #[test]
    fn round_trips_through_text() {
        let chosen = Preferences {
            hardware_decoding: false,
            autosave: false,
            autosave_minutes: 12,
            scrub_audio: false,
            dropped_frame_indicator: true,
            audio_device: Some("Speakers (USB Audio)".to_owned()),
        };
        assert_eq!(DEFAULTS.read(&chosen.to_text()), chosen);
    }

    #[test]
    fn unreadable_and_unknown_lines_keep_what_was_there() {
        let text = "autosave_minutes\tlots\nscrub_audio\tfalse\nfuture_choice\ttrue\njunk\n";
        let read = DEFAULTS.read(text);
        assert_eq!(
            read,
            Preferences {
                scrub_audio: false,
                ..DEFAULTS
            }
        );
    }
}
