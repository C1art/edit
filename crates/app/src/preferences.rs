use audio::{InputDevice, OutputDevice};

use crate::editor::Editor;
use crate::project_store::Setting;
use crate::proxy_offer::HeavyMedia;

/// The choices kept from one session to the next, written as `name<TAB>value` lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Preferences {
    hardware_decoding: bool,
    autosave: bool,
    autosave_minutes: u32,
    scrub_audio: bool,
    selection_follows_playhead: bool,
    dropped_frame_indicator: bool,
    heavy_media: HeavyMedia,
    /// `None` follows the system default.
    audio_device: Option<OutputDevice>,
    /// `None` records from the system default.
    voiceover_input: Option<InputDevice>,
    voiceover_countdown: bool,
    voiceover_hear_sequence: bool,
}

impl Preferences {
    fn of(editor: &Editor) -> Self {
        Self {
            hardware_decoding: editor.hardware_decoding,
            autosave: editor.autosave.enabled,
            autosave_minutes: editor.autosave.minutes,
            scrub_audio: editor.scrub_audio,
            selection_follows_playhead: editor.selection_follows_playhead,
            dropped_frame_indicator: editor.dropped_frame_indicator,
            heavy_media: editor.proxy_offer.choice,
            audio_device: editor.audio_switch.chosen.clone(),
            voiceover_input: editor.voiceover.input.clone(),
            voiceover_countdown: editor.voiceover.countdown,
            voiceover_hear_sequence: editor.voiceover.hear_sequence,
        }
    }

    pub(crate) fn to_text(&self) -> String {
        format!(
            "hardware_decoding\t{}\nautosave\t{}\nautosave_minutes\t{}\nscrub_audio\t{}\n\
             selection_follows_playhead\t{}\ndropped_frame_indicator\t{}\nheavy_media\t{}\n\
             audio_device\t{}\naudio_device_id\t{}\nvoiceover_input\t{}\n\
             voiceover_input_id\t{}\nvoiceover_countdown\t{}\nvoiceover_hear_sequence\t{}\n",
            self.hardware_decoding,
            self.autosave,
            self.autosave_minutes,
            self.scrub_audio,
            self.selection_follows_playhead,
            self.dropped_frame_indicator,
            self.heavy_media.key(),
            self.audio_device
                .as_ref()
                .map_or("", |device| device.name.as_str()),
            self.audio_device
                .as_ref()
                .and_then(|device| device.id.as_deref())
                .unwrap_or_default(),
            self.voiceover_input
                .as_ref()
                .map_or("", |device| device.name.as_str()),
            self.voiceover_input
                .as_ref()
                .and_then(|device| device.id.as_deref())
                .unwrap_or_default(),
            self.voiceover_countdown,
            self.voiceover_hear_sequence,
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
                "selection_follows_playhead" => set(&mut self.selection_follows_playhead, flag),
                "dropped_frame_indicator" => set(&mut self.dropped_frame_indicator, flag),
                "heavy_media" => set(&mut self.heavy_media, HeavyMedia::from_key(value)),
                "audio_device" => {
                    self.audio_device = (!value.is_empty()).then(|| OutputDevice {
                        id: None,
                        name: value.to_owned(),
                    });
                }
                // Written after the name, so it lands on the device just read.
                "audio_device_id" => {
                    if let Some(device) = &mut self.audio_device {
                        device.id = (!value.is_empty()).then(|| value.to_owned());
                    }
                }
                "voiceover_input" => {
                    self.voiceover_input = (!value.is_empty()).then(|| InputDevice {
                        id: None,
                        name: value.to_owned(),
                    });
                }
                "voiceover_input_id" => {
                    if let Some(device) = &mut self.voiceover_input {
                        device.id = (!value.is_empty()).then(|| value.to_owned());
                    }
                }
                "voiceover_countdown" => set(&mut self.voiceover_countdown, flag),
                "voiceover_hear_sequence" => set(&mut self.voiceover_hear_sequence, flag),
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
        self.selection_follows_playhead = preferences.selection_follows_playhead;
        self.dropped_frame_indicator = preferences.dropped_frame_indicator;
        self.proxy_offer.choice = preferences.heavy_media;
        self.audio_switch.chosen = preferences.audio_device;
        self.voiceover.input = preferences.voiceover_input;
        self.voiceover.countdown = preferences.voiceover_countdown;
        self.voiceover.hear_sequence = preferences.voiceover_hear_sequence;
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
        selection_follows_playhead: false,
        dropped_frame_indicator: false,
        heavy_media: HeavyMedia::Ask,
        audio_device: None,
        voiceover_input: None,
        voiceover_countdown: true,
        voiceover_hear_sequence: false,
    };

    #[test]
    fn round_trips_through_text() {
        let chosen = Preferences {
            hardware_decoding: false,
            autosave: false,
            autosave_minutes: 12,
            scrub_audio: false,
            selection_follows_playhead: true,
            dropped_frame_indicator: true,
            heavy_media: HeavyMedia::CreateProxies,
            audio_device: Some(OutputDevice {
                id: Some(
                    "wasapi:{0.0.0.00000000}.{6c1f2a52-3b8e-4f0a-9d3c-0a1b2c3d4e5f}".to_owned(),
                ),
                name: "Speakers (USB Audio)".to_owned(),
            }),
            voiceover_input: Some(InputDevice {
                id: Some("wasapi:{0.0.1.00000000}.{1a2b3c4d}".to_owned()),
                name: "Microphone (USB Audio)".to_owned(),
            }),
            voiceover_countdown: false,
            voiceover_hear_sequence: true,
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
