use playback::PlayRate;
use timeline::{Gain, Param, SetTrackAutomation, TrackRef};

use super::automation::{AutomationMode, Pass};
use super::{live, Mixer};
use crate::editor::Editor;

pub(super) fn mode_picker(ui: &mut egui::Ui, mixer: &mut Mixer, track: TrackRef) {
    let mut mode = mixer.mode(track);
    egui::ComboBox::from_id_salt(("automation mode", track))
        .width(60.0)
        .wrap_mode(egui::TextWrapMode::Extend)
        .selected_text(mode.name())
        .show_ui(ui, |ui| {
            for option in AutomationMode::ALL {
                ui.selectable_value(&mut mode, option, option.name())
                    .on_hover_text(option.tip());
            }
        })
        .response
        .on_hover_text(mode.tip());
    mixer.modes.insert(track, mode);
}

/// Records the fader into the track's volume keyframes while its mode says to, and
/// commits a pass once it ends. True while the fader is being recorded, and on the
/// frame its pass ends.
pub(super) fn automate(
    editor: &mut Editor,
    track: TrackRef,
    volume: Gain,
    db: f32,
    touched: bool,
) -> bool {
    let playing = editor.program.as_ref().is_some_and(|program| {
        program.player.is_playing() && program.player.rate() > PlayRate::STOPPED
    });
    let playhead = editor.playhead();
    let last = editor.mixer.passes.get(&track).map(Pass::last);
    // Looping or seeking back starts a new pass rather than writing over this one.
    let rewound = last.flatten().is_some_and(|(time, _)| playhead < time);
    let mode = editor.mixer.mode(track);
    let records =
        playing && volume > Gain::SILENT && mode.records(last.is_some() && !rewound, touched);
    if last.is_some() && (rewound || !records) {
        finish_pass(editor, track);
    }
    if !records {
        // Releasing the fader in Touch ends the pass on the frame the drag stops, and
        // that release mustn't also set the track's own volume.
        return last.is_some();
    }
    let level = db - volume.db();
    let pass = editor.mixer.passes.entry(track).or_default();
    let heard = pass.last().map(|(_, previous)| previous);
    pass.record(playhead, level);
    if heard != Some(level) {
        let automation = Param::from_value(level);
        live(editor, &SetTrackAutomation { track, automation });
    }
    true
}

fn finish_pass(editor: &mut Editor, track: TrackRef) {
    let Some(pass) = editor.mixer.passes.remove(&track) else {
        return;
    };
    let sequence = editor.timeline.sequence();
    let Some(current) = sequence.track(track) else {
        return;
    };
    let automation = pass.spliced(&current.automation, sequence.timebase);
    if automation == current.automation {
        // Nothing to commit, but the mix still holds the pass's level.
        editor.mix_program(sequence.clone());
        return;
    }
    editor.apply_levels(SetTrackAutomation { track, automation });
}
