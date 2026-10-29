use std::collections::VecDeque;

use timeline::{Ticks, Timebase};

use crate::AudioBuffer;

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
    held_video: Option<(Ticks, P)>,
    ready: VecDeque<Trimmed<P>>,
}

impl<P> SeekTrim<P> {
    pub(crate) fn new(timebase: Timebase) -> Self {
        Self {
            timebase,
            video_target: None,
            audio_target: None,
            held_video: None,
            ready: VecDeque::new(),
        }
    }

    pub(crate) fn reset(&mut self, target: Ticks) {
        self.video_target = Some(target);
        self.audio_target = Some(target);
        self.held_video = None;
        self.ready.clear();
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

    pub(crate) fn push_audio(&mut self, mut buffer: AudioBuffer) {
        if let Some(target) = self.audio_target {
            if buffer.end(self.timebase) <= target {
                return;
            }
            if buffer.pts < target {
                buffer.trim_start_to(target, self.timebase);
            }
            self.audio_target = None;
        }
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
}
