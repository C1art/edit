//! Gang Source and Program: moving either monitor's playhead moves the other's by as
//! much, so two takes or a take and the cut can be stepped through side by side.

use timeline::{SourceId, Ticks};

use crate::editor::Editor;

/// Where both playheads were when last in step, and what each monitor showed then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Gang {
    source: SourceId,
    nest: Option<SourceId>,
    source_at: Ticks,
    program_at: Ticks,
}

impl Editor {
    pub(crate) fn toggle_gang(&mut self) {
        self.ganged = !self.ganged;
        self.gang = None;
    }

    /// Carries a move of one playhead over to the other. A playing monitor is followed
    /// once it stops, so playback doesn't make the other seek every frame.
    pub(crate) fn follow_gang(&mut self) {
        if !self.ganged {
            return;
        }
        let nest = self.timeline.open_nest();
        let (Some(source), Some(program)) = (&mut self.source, &mut self.program) else {
            self.gang = None;
            return;
        };
        let (source_player, program_player) = (&mut source.viewer.player, &mut program.player);
        if source_player.is_playing() || program_player.is_playing() {
            return;
        }
        let (source_at, program_at) = (source_player.position(), program_player.position());
        let in_step = self
            .gang
            .filter(|gang| gang.source == source.source && gang.nest == nest);
        if let Some(gang) = in_step {
            let source_moved = source_at - gang.source_at;
            let program_moved = program_at - gang.program_at;
            match (source_moved == Ticks::ZERO, program_moved == Ticks::ZERO) {
                (false, true) => program_player.seek(program_at + source_moved),
                (true, false) => source_player.seek(source_at + program_moved),
                _ => {}
            }
        }
        self.gang = Some(Gang {
            source: source.source,
            nest,
            source_at: source_player.position(),
            program_at: program_player.position(),
        });
    }
}
