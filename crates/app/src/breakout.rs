//! Breakout to Mono: a project item for each of a file's channels, once the file says
//! how many it has.

use std::time::Duration;

use timeline::{BreakoutToMono, SourceId};

use crate::editor::Editor;
use crate::metadata_panel::Channels;

impl Editor {
    pub(crate) fn breakout_to_mono(&mut self, source: SourceId) {
        let Some(item) = self.timeline.project().media_item(source) else {
            return;
        };
        if !(item.has_audio && item.is_file()) || self.breakouts.contains(&source) {
            return;
        }
        let path = item.path.clone();
        self.metadata.audio_channels(&path);
        self.breakouts.push(source);
    }

    /// Breaks out the items whose files have been read since asked.
    pub(crate) fn follow_breakouts(&mut self, ctx: &egui::Context) {
        if self.breakouts.is_empty() {
            return;
        }
        self.metadata.poll();
        let mut pending = std::mem::take(&mut self.breakouts);
        pending.retain(|&source| {
            let Some(item) = self.timeline.project().media_item(source) else {
                return false;
            };
            let name = item.name.clone();
            let path = item.path.clone();
            match self.metadata.audio_channels(&path) {
                Channels::Reading => return true,
                Channels::Read(channels) if channels > 1 => {
                    self.apply_to_pool(BreakoutToMono { source, channels });
                }
                Channels::Read(_) => {
                    self.notice(format!("\u{201C}{name}\u{201D} has only one channel"));
                }
                Channels::Failed(error) => {
                    self.notice(format!(
                        "Cannot read the channels of \u{201C}{name}\u{201D}: {error}"
                    ));
                }
            }
            false
        });
        if !pending.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        self.breakouts = pending;
    }
}
