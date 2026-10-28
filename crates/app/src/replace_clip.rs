use timeline::ReplaceClip;

use crate::editor::Editor;

impl Editor {
    /// Puts the Source Monitor's media under the first selected clip, as Premiere's
    /// Replace With Clip. It reads from the source in point, or the source playhead;
    /// with `match_frame`, the source playhead's frame lands under the sequence playhead.
    pub(crate) fn replace_with_source(&mut self, match_frame: bool) {
        let Some(source) = &self.source else {
            self.notice("Open a clip in the Source Monitor to replace with");
            return;
        };
        let sequence = self.timeline.sequence();
        let Some((_, clip)) = self
            .view
            .selection
            .first()
            .and_then(|&id| sequence.clip(id))
        else {
            self.notice("Select a clip to replace");
            return;
        };
        let position = source.viewer.player.position();
        let source_in = if match_frame {
            let playhead = self.playhead();
            if !(clip.start..clip.end()).contains(&playhead) {
                self.notice("Put the playhead over the clip to match frames");
                return;
            }
            let into_clip = clip.source_time(playhead) - clip.source_in;
            source.media_range(position..position).0 - into_clip
        } else {
            let start = source.marks.mark_in.unwrap_or(position);
            source.media_range(start..start).0
        };
        self.apply(ReplaceClip {
            clip: clip.id,
            source: source.source,
            source_in,
        });
    }
}
