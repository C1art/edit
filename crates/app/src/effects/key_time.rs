//! Which source time a clip's keyframes are set at from the playhead, and where the
//! playhead goes to show one.

use std::ops::RangeInclusive;

use timeline::{Clip, FrameRate, Ticks, Timebase};

use crate::editor::Editor;

/// The source time keyframes go to: what the playhead's frame shows, kept within the
/// clip. A keyframe anywhere in the source that frame shows counts as being there, as
/// keyframes set before a change of speed rarely fall on a frame's first tick, and
/// keying beside one would jerk the value within a frame.
pub(crate) fn keyframe_time(editor: &Editor, clip: &Clip) -> Ticks {
    let sequence = editor.timeline.sequence();
    let frames = (sequence.format.frame_rate, sequence.timebase);
    key_time_at(clip, editor.playhead(), frames)
}

pub(super) fn seek_to_key(editor: &mut Editor, clip: &Clip, key: Option<Ticks>) {
    if let Some(key) = key {
        let sequence = editor.timeline.sequence();
        let frames = (sequence.format.frame_rate, sequence.timebase);
        editor.seek_program(frame_showing(clip, key, frames));
    }
}

fn key_time_at(clip: &Clip, playhead: Ticks, frames: (FrameRate, Timebase)) -> Ticks {
    let playhead = playhead.clamp(clip.start, clip.end() - Ticks(1));
    let time = clip.source_time(playhead);
    let shown = frame_source(clip, frames.0.frame_at(playhead, frames.1), frames);
    clip.look
        .params()
        .flat_map(|param| param.keys.iter().map(|key| key.time))
        .filter(|key| shown.contains(key))
        .min_by_key(|&key| (key - time).0.abs())
        .unwrap_or(time)
}

/// The start of the frame showing source time `key`, within the clip.
fn frame_showing(clip: &Clip, key: Ticks, (rate, timebase): (FrameRate, Timebase)) -> Ticks {
    let position = clip
        .position_of(key)
        .clamp(clip.start, clip.end() - Ticks(1));
    let frame = rate.frame_at(position, timebase);
    // Positions round down, which can land a tick short of the frame showing the key.
    let frame = [frame, frame + 1]
        .into_iter()
        .find(|&frame| frame_source(clip, frame, (rate, timebase)).contains(&key))
        .unwrap_or(frame);
    rate.frame_start(frame, timebase)
        .clamp(clip.start, clip.end() - Ticks(1))
}

/// The source sequence frame `frame` shows of `clip`.
fn frame_source(
    clip: &Clip,
    frame: i64,
    (rate, timebase): (FrameRate, Timebase),
) -> RangeInclusive<Ticks> {
    let first = rate.frame_start(frame, timebase).max(clip.start);
    let last = (rate.frame_start(frame + 1, timebase).min(clip.end()) - Ticks(1)).max(first);
    let [from, to] = [first, last].map(|position| clip.source_time(position));
    from.min(to)..=from.max(to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use timeline::{ClipId, SourceId, Speed};

    #[test]
    fn a_key_off_the_frame_start_is_found_and_shown_after_a_speed_change() {
        let frames = (FrameRate::FPS_25, Timebase::HZ_90K);
        let frame = 3_600;
        let mut clip = Clip::new(
            ClipId(1),
            SourceId(1),
            Ticks::ZERO,
            Ticks(frame * 50),
            Ticks::ZERO,
        );
        clip.speed = Speed::from_basis_points(13_333).unwrap();
        // Set at speed 100% on frame 10's first tick, which now plays partway into a frame.
        let key = Ticks(frame * 10);
        clip.look.motion.scale.toggle_key(key);
        let shown = frame_showing(&clip, key, frames);
        assert!(frame_source(&clip, frames.0.frame_at(shown, frames.1), frames).contains(&key));
        assert_eq!(
            shown,
            frames
                .0
                .frame_start(frames.0.frame_at(shown, frames.1), frames.1)
        );
        assert_eq!(key_time_at(&clip, shown, frames), key);
        assert_ne!(clip.source_time(shown), key);
    }
}
