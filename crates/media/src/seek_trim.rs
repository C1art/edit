use std::collections::VecDeque;

use timeline::{Ticks, Timebase};

use crate::AudioBuffer;

/// Audio stamped this far from where the last buffer ended is taken as following on
/// from it: stamps are rounded, and a lost packet leaves a far larger gap.
const AUDIO_SLACK: Timebase = match Timebase::new(1, 200) {
    Some(timebase) => timebase,
    None => Timebase::HZ_90K,
};
/// The most silence put in one gap, in seconds: more is a broken stamp rather than lost
/// sound.
const MAX_GAP: (Ticks, Timebase) = match Timebase::new(1, 1) {
    Some(seconds) => (Ticks(10), seconds),
    None => (Ticks(900_000), Timebase::HZ_90K),
};

/// What a seek keeps: pictures, still as the decoder left them, and sound.
pub(crate) enum Trimmed<P> {
    Video(Ticks, P),
    Audio(AudioBuffer),
}

/// Makes seeks frame-accurate. Demuxers land on the keyframe before the target, so the
/// frames decoded between that keyframe and the target are dropped here, except for the
/// picture that is on screen at the target and the audio from the target onwards.
pub(crate) struct SeekTrim<P> {
    timebase: Timebase,
    video_target: Option<Ticks>,
    audio_target: Option<Ticks>,
    /// Where the audio passed on so far ends.
    audio_end: Option<Ticks>,
    held_video: Option<(Ticks, P)>,
    ready: VecDeque<Trimmed<P>>,
}

impl<P> SeekTrim<P> {
    pub(crate) fn new(timebase: Timebase) -> Self {
        Self {
            timebase,
            video_target: None,
            audio_target: None,
            // Read from the start, the sound starts at the media's start too.
            audio_end: Some(Ticks::ZERO),
            held_video: None,
            ready: VecDeque::new(),
        }
    }

    pub(crate) fn reset(&mut self, target: Ticks) {
        self.video_target = Some(target);
        self.audio_target = Some(target);
        self.audio_end = None;
        self.held_video = None;
        self.ready.clear();
    }

    /// Moves on to a later target without a seek, for decoding on to it. Frames decoded
    /// but not yet taken are trimmed as if they had come after a seek.
    pub(crate) fn advance(&mut self, target: Ticks) {
        let held = self.held_video.take();
        let ready = std::mem::take(&mut self.ready);
        self.video_target = Some(target);
        self.audio_target = Some(target);
        self.audio_end = None;
        if let Some((pts, picture)) = held {
            self.push_video(pts, picture);
        }
        for frame in ready {
            match frame {
                Trimmed::Video(pts, picture) => self.push_video(pts, picture),
                Trimmed::Audio(buffer) => self.push_audio(buffer),
            }
        }
    }

    pub(crate) fn push_video(&mut self, pts: Ticks, picture: P) {
        let Some(target) = self.video_target else {
            self.ready.push_back(Trimmed::Video(pts, picture));
            return;
        };
        if pts < target {
            self.held_video = Some((pts, picture));
            return;
        }
        if pts > target {
            if let Some((held_pts, held)) = self.held_video.take() {
                self.ready.push_back(Trimmed::Video(held_pts, held));
            }
        }
        self.video_target = None;
        self.held_video = None;
        self.ready.push_back(Trimmed::Video(pts, picture));
    }

    /// Passes on audio from the target onwards, with silence where the stream starts
    /// after the target or skips ahead, and without what it plays twice, so that each
    /// sample plays when its stamp says, as each picture does.
    pub(crate) fn push_audio(&mut self, mut buffer: AudioBuffer) {
        let timebase = self.timebase;
        let (expected, slack) = match (self.audio_target, self.audio_end) {
            (Some(target), _) => (target, Ticks::ZERO),
            (None, Some(end)) => (end, Ticks(1).rescale(AUDIO_SLACK, timebase)),
            (None, None) => (buffer.pts, Ticks::ZERO),
        };
        if buffer.end(timebase) <= expected {
            return;
        }
        self.audio_target = None;
        let (max_gap, seconds) = MAX_GAP;
        if buffer.pts + slack < expected {
            buffer.trim_start_to(expected, timebase);
        } else if buffer.pts > expected + slack
            && buffer.pts - expected <= max_gap.rescale(seconds, timebase)
        {
            if let Some(silence) = buffer.silence_before(expected, timebase) {
                self.ready.push_back(Trimmed::Audio(silence));
            }
        }
        self.audio_end = Some(buffer.end(self.timebase));
        self.ready.push_back(Trimmed::Audio(buffer));
    }

    /// Releases the last picture when the seek target lies past the final frame.
    pub(crate) fn finish(&mut self) {
        if let Some((pts, held)) = self.held_video.take() {
            self.ready.push_back(Trimmed::Video(pts, held));
        }
        self.video_target = None;
        self.audio_target = None;
    }

    pub(crate) fn pop(&mut self) -> Option<Trimmed<P>> {
        self.ready.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tb() -> Timebase {
        Timebase::new(1, 1000).unwrap()
    }

    fn audio(pts: i64, frames: usize) -> AudioBuffer {
        AudioBuffer {
            pts: Ticks(pts),
            sample_rate: 1000,
            channels: 2,
            samples: (0..frames * 2).map(|i| i as f32).collect(),
        }
    }

    fn drain(trim: &mut SeekTrim<()>) -> Vec<Trimmed<()>> {
        std::iter::from_fn(|| trim.pop()).collect()
    }

    fn video_pts(frames: &[Trimmed<()>]) -> Vec<i64> {
        frames
            .iter()
            .filter_map(|frame| match frame {
                Trimmed::Video(pts, ()) => Some(pts.0),
                Trimmed::Audio(_) => None,
            })
            .collect()
    }

    #[test]
    fn keeps_the_frame_on_screen_at_a_target_between_frames() {
        let mut trim = SeekTrim::<()>::new(tb());
        trim.reset(Ticks(50));
        for pts in [0, 40, 80, 120] {
            trim.push_video(Ticks(pts), ());
        }
        assert_eq!(video_pts(&drain(&mut trim)), [40, 80, 120]);
    }

    #[test]
    fn drops_the_previous_frame_when_a_frame_starts_exactly_at_target() {
        let mut trim = SeekTrim::<()>::new(tb());
        trim.reset(Ticks(80));
        for pts in [0, 40, 80, 120] {
            trim.push_video(Ticks(pts), ());
        }
        assert_eq!(video_pts(&drain(&mut trim)), [80, 120]);
    }

    #[test]
    fn advancing_trims_frames_already_decoded_and_keeps_the_one_on_screen() {
        let mut trim = SeekTrim::<()>::new(tb());
        trim.reset(Ticks(0));
        for pts in [0, 40, 80] {
            trim.push_video(Ticks(pts), ());
        }
        trim.push_audio(audio(0, 120));
        trim.advance(Ticks(100));
        trim.push_video(Ticks(120), ());
        let frames = drain(&mut trim);
        assert_eq!(video_pts(&frames), [80, 120]);
        let audio_starts: Vec<i64> = frames
            .iter()
            .filter_map(|frame| match frame {
                Trimmed::Audio(buffer) => Some(buffer.pts.0),
                Trimmed::Video(..) => None,
            })
            .collect();
        assert_eq!(audio_starts, [100]);
    }

    #[test]
    fn seek_past_the_last_frame_shows_the_last_frame_on_finish() {
        let mut trim = SeekTrim::<()>::new(tb());
        trim.reset(Ticks(500));
        trim.push_video(Ticks(0), ());
        trim.push_video(Ticks(40), ());
        assert!(trim.pop().is_none());
        trim.finish();
        assert_eq!(video_pts(&drain(&mut trim)), [40]);
    }

    #[test]
    fn trims_audio_to_start_exactly_at_target() {
        let mut trim = SeekTrim::<()>::new(tb());
        trim.reset(Ticks(105));
        trim.push_audio(audio(0, 100));
        trim.push_audio(audio(100, 100));
        trim.push_audio(audio(200, 100));
        let out = drain(&mut trim);
        let [Trimmed::Audio(first), Trimmed::Audio(second)] = out.as_slice() else {
            panic!("expected two audio buffers");
        };
        assert_eq!(first.pts, Ticks(105));
        assert_eq!(first.frames(), 95);
        assert_eq!(first.samples[0], 10.0);
        assert_eq!(second.pts, Ticks(200));
    }

    #[test]
    fn audio_starting_after_the_target_is_preceded_by_silence() {
        let mut trim = SeekTrim::<()>::new(tb());
        trim.reset(Ticks(100));
        trim.push_audio(audio(150, 100));
        let out = drain(&mut trim);
        let [Trimmed::Audio(silence), Trimmed::Audio(sound)] = out.as_slice() else {
            panic!("expected silence then sound");
        };
        assert_eq!((silence.pts, silence.frames()), (Ticks(100), 50));
        assert!(silence.samples.iter().all(|&sample| sample == 0.0));
        assert_eq!(sound.pts, Ticks(150));
    }

    #[test]
    fn a_gap_in_the_audio_is_filled_and_an_overlap_dropped() {
        let mut trim = SeekTrim::<()>::new(tb());
        trim.reset(Ticks(0));
        trim.push_audio(audio(0, 100));
        trim.push_audio(audio(130, 100));
        trim.push_audio(audio(210, 100));
        let out = drain(&mut trim);
        let spans: Vec<(i64, usize)> = out
            .iter()
            .filter_map(|frame| match frame {
                Trimmed::Audio(buffer) => Some((buffer.pts.0, buffer.frames())),
                Trimmed::Video(..) => None,
            })
            .collect();
        assert_eq!(spans, [(0, 100), (100, 30), (130, 100), (230, 80)]);
    }
}
