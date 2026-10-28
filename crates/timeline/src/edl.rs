//! CMX 3600 edit decision lists, the plain-text interchange other editors and finishing
//! systems read: one video track and the first two audio tracks, as cuts, dissolves,
//! wipes and fades through black, with speed changes as M2 lines.

use std::fmt::Write;

use crate::plan::cut_window;
use crate::{Clip, FrameRate, Sequence, Ticks, Timecode, Track, TransitionKind};

const BLACK: &str = "BL";
/// Media is named in comments rather than reels, which files don't have.
const REEL: &str = "AX";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    Cut,
    Dissolve(i64),
    Wipe(i64),
}

#[derive(Debug, Clone, PartialEq)]
struct Line {
    reel: &'static str,
    change: Change,
    /// Frames counted at `source_rate`, which media keeps from its own file.
    source: (i64, i64),
    source_rate: FrameRate,
    record: (i64, i64),
}

#[derive(Debug, Clone, PartialEq)]
struct Event {
    lines: Vec<Line>,
    /// Frames per second the source plays at, when not its own.
    speed: Option<f64>,
    comments: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
struct Channels {
    video: bool,
    first: bool,
    second: bool,
}

impl Channels {
    /// The CMX name for these channels together; none for a pairing it can't express.
    fn label(self) -> Option<&'static str> {
        Some(match (self.video, self.first, self.second) {
            (true, false, false) => "V",
            (false, true, false) => "A",
            (false, false, true) => "A2",
            (false, true, true) => "AA",
            (true, true, false) => "B",
            (true, true, true) => "AA/V",
            _ => return None,
        })
    }

    fn with(self, other: Self) -> Self {
        Self {
            video: self.video || other.video,
            first: self.first || other.first,
            second: self.second || other.second,
        }
    }
}

/// The edit decision list of `sequence`'s video track `video_track` and its first two
/// audio tracks. Disabled clips are left out.
pub fn to_edl(sequence: &Sequence, title: &str, video_track: usize) -> String {
    let tracks = [
        (
            sequence.video.get(video_track),
            Channels {
                video: true,
                ..Default::default()
            },
        ),
        (
            sequence.audio.first(),
            Channels {
                first: true,
                ..Default::default()
            },
        ),
        (
            sequence.audio.get(1),
            Channels {
                second: true,
                ..Default::default()
            },
        ),
    ];
    let mut events: Vec<(Event, Channels)> = Vec::new();
    for (track, channels) in tracks {
        let Some(track) = track else { continue };
        for event in track_events(sequence, track, channels.video) {
            let merged = events.iter_mut().find(|(other, other_channels)| {
                *other == event && other_channels.with(channels).label().is_some()
            });
            match merged {
                Some((_, other_channels)) => *other_channels = other_channels.with(channels),
                None => events.push((event, channels)),
            }
        }
    }
    events.sort_by_key(|(event, channels)| {
        let first = event.lines.first().map_or(0, |line| line.record.0);
        (first, !channels.video)
    });
    let rate = sequence.format.frame_rate;
    let timecode = |frame: i64, rate: FrameRate| {
        let position = rate.frame_start(frame, sequence.timebase);
        Timecode::new(position, sequence.timebase, rate).to_string()
    };
    let mut text = format!("TITLE: {title}\nFCM: NON-DROP FRAME\n");
    for (number, (event, channels)) in events.iter().enumerate() {
        let number = number + 1;
        let label = channels.label().unwrap_or("V");
        text.push('\n');
        for line in &event.lines {
            let (kind, duration) = match line.change {
                Change::Cut => ("C".to_owned(), String::new()),
                Change::Dissolve(frames) => ("D".to_owned(), format!("{frames:03}")),
                Change::Wipe(frames) => ("W001".to_owned(), format!("{frames:03}")),
            };
            let _ = writeln!(
                text,
                "{number:03}  {:<8} {label:<5} {kind:<4} {duration:>3} {} {} {} {}",
                line.reel,
                timecode(line.source.0, line.source_rate),
                timecode(line.source.1, line.source_rate),
                timecode(line.record.0, rate),
                timecode(line.record.1, rate),
            );
        }
        if let (Some(fps), Some(line)) = (event.speed, event.lines.last()) {
            let speed = if fps < 0.0 {
                format!("-{:05.1}", -fps)
            } else {
                format!("{fps:05.1}")
            };
            let _ = writeln!(
                text,
                "M2   {REEL:<8}       {speed}                {}",
                timecode(line.source.0, line.source_rate)
            );
        }
        for comment in &event.comments {
            let _ = writeln!(text, "* {comment}");
        }
    }
    text
}

/// A clip's span in frames, of the sequence or of its source.
struct Frames<'a> {
    sequence: &'a Sequence,
}

impl Frames<'_> {
    fn record(&self, position: Ticks) -> i64 {
        let rate = self.sequence.format.frame_rate;
        rate.frame_at(position, self.sequence.timebase)
    }

    /// Source timecode counts the media's own frames, at the sequence rate for sound.
    fn source_rate(&self, clip: &Clip) -> FrameRate {
        self.sequence
            .media_item(clip.source)
            .and_then(|item| item.video)
            .map_or(self.sequence.format.frame_rate, |video| video.frame_rate)
    }

    fn source(&self, clip: &Clip, position: Ticks) -> i64 {
        let time = clip.source_time(position).max(Ticks::ZERO);
        self.source_rate(clip)
            .frame_at(time, self.sequence.timebase)
    }

    /// The line playing `clip` over `record`, its source running as long as the record
    /// does when the clip plays off speed, as CMX writes it.
    fn line(&self, clip: &Clip, change: Change, record: (Ticks, Ticks)) -> Line {
        let (start, end) = (self.record(record.0), self.record(record.1));
        let at = |frame| {
            let rate = self.sequence.format.frame_rate;
            self.source(clip, rate.frame_start(frame, self.sequence.timebase))
        };
        let source_in = at(start);
        let source_out = match speed(clip) {
            Some(_) => source_in + (end - start),
            None => source_in.max(at(end)),
        };
        Line {
            reel: REEL,
            change,
            source: (source_in, source_out),
            source_rate: self.source_rate(clip),
            record: (start, end),
        }
    }

    /// A line holding `clip` at `position` for no time, where a transition starts.
    fn still(&self, clip: Option<&Clip>, position: Ticks) -> Line {
        let record = self.record(position);
        let (reel, source, source_rate) = match clip {
            Some(clip) => (REEL, self.source(clip, position), self.source_rate(clip)),
            None => (BLACK, 0, self.sequence.format.frame_rate),
        };
        Line {
            reel,
            change: Change::Cut,
            source: (source, source),
            source_rate,
            record: (record, record),
        }
    }
}

/// How fast `clip` plays its source, when not at its own speed: negative in reverse,
/// zero for a still.
fn speed(clip: &Clip) -> Option<f64> {
    if clip.hold {
        return Some(0.0);
    }
    let span = (clip.source_out() - clip.source_in).0 as f64;
    let rate = span / clip.duration.0.max(1) as f64;
    let rate = if clip.reverse { -rate } else { rate };
    ((rate - 1.0).abs() > 1e-6).then_some(rate)
}

fn change(kind: TransitionKind, video: bool, frames: i64) -> Change {
    let dissolves = matches!(
        kind,
        TransitionKind::CrossDissolve | TransitionKind::DipToBlack | TransitionKind::DipToWhite
    );
    if dissolves || !video {
        Change::Dissolve(frames)
    } else {
        Change::Wipe(frames)
    }
}

fn track_events(sequence: &Sequence, track: &Track, video: bool) -> Vec<Event> {
    let frames = Frames { sequence };
    let clips: Vec<&Clip> = track.clips.iter().filter(|clip| !clip.disabled).collect();
    let name = |clip: &Clip| {
        sequence
            .media_item(clip.source)
            .map_or_else(String::new, |item| item.name.clone())
    };
    let mut events = Vec::new();
    for (index, &clip) in clips.iter().enumerate() {
        let previous = index
            .checked_sub(1)
            .map(|before| clips[before])
            .filter(|previous| previous.end() == clip.start);
        let next = clips.get(index + 1).filter(|next| next.start == clip.end());
        let mut end = clip.end();
        if let Some((next, transition)) = next.and_then(|next| Some((next, next.transition_in?))) {
            end = clip.end() - cut_window(clip, next, transition).0;
        }
        let fade_out = clip.transition_out.filter(|_| next.is_none());
        if let Some(transition) = fade_out {
            end = clip.end() - transition.duration.min(clip.duration);
        }
        let mut comments = Vec::new();
        let lines = match (clip.transition_in, previous) {
            (Some(transition), Some(previous)) => {
                let (before, after) = cut_window(previous, clip, transition);
                let start = clip.start - before;
                let length = frames.record(clip.start + after) - frames.record(start);
                comments.push(format!("FROM CLIP NAME: {}", name(previous)));
                comments.push(format!("TO CLIP NAME: {}", name(clip)));
                vec![
                    frames.still(Some(previous), start),
                    frames.line(clip, change(transition.kind, video, length), (start, end)),
                ]
            }
            (Some(transition), None) => {
                let length = transition.duration.min(clip.duration);
                let length = frames.record(clip.start + length) - frames.record(clip.start);
                comments.push(format!("TO CLIP NAME: {}", name(clip)));
                vec![
                    frames.still(None, clip.start),
                    frames.line(
                        clip,
                        change(transition.kind, video, length),
                        (clip.start, end),
                    ),
                ]
            }
            (None, _) => {
                comments.push(format!("FROM CLIP NAME: {}", name(clip)));
                vec![frames.line(clip, Change::Cut, (clip.start, end))]
            }
        };
        let speed = speed(clip).map(|rate| frames.source_rate(clip).approx_fps() * rate);
        events.push(Event {
            lines,
            speed,
            comments,
        });
        if let Some(transition) = fade_out {
            let record = (frames.record(end), frames.record(clip.end()));
            let length = record.1 - record.0;
            events.push(Event {
                lines: vec![
                    frames.still(Some(clip), end),
                    Line {
                        reel: BLACK,
                        change: change(transition.kind, video, length),
                        source: (0, length),
                        source_rate: sequence.format.frame_rate,
                        record,
                    },
                ],
                speed: None,
                comments: vec![format!("FROM CLIP NAME: {}", name(clip))],
            });
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClipId, SourceId, Speed, Timebase, Transition, VideoFormat};

    /// Ticks are frames, so positions read as frame counts.
    fn sequence(video: Vec<Clip>, audio: Vec<Clip>) -> Sequence {
        let timebase = Timebase::new(1, 25).expect("valid timebase");
        let mut sequence = Sequence::new(timebase, VideoFormat::default());
        sequence.video = vec![Track {
            clips: video,
            ..Track::default()
        }];
        sequence.audio = vec![Track {
            clips: audio,
            ..Track::default()
        }];
        sequence
    }

    fn clip(id: u64, start: i64, duration: i64, source_in: i64) -> Clip {
        Clip::new(
            ClipId(id),
            SourceId(1),
            Ticks(start),
            Ticks(duration),
            Ticks(source_in),
        )
    }

    fn events(edl: &str) -> Vec<&str> {
        edl.lines()
            .filter(|line| !line.starts_with('*') && !line.is_empty())
            .skip(2)
            .collect()
    }

    #[test]
    fn matching_picture_and_sound_cuts_share_an_event() {
        let edl = to_edl(
            &sequence(vec![clip(1, 25, 50, 100)], vec![clip(2, 25, 50, 100)]),
            "Cut",
            0,
        );
        assert!(edl.starts_with("TITLE: Cut\nFCM: NON-DROP FRAME\n"));
        assert_eq!(
            events(&edl),
            ["001  AX       B     C        00:00:04:00 00:00:06:00 00:00:01:00 00:00:03:00"]
        );
    }

    #[test]
    fn a_dissolve_holds_the_outgoing_clip_then_dissolves_across_the_cut() {
        let mut incoming = clip(2, 50, 50, 200);
        incoming.transition_in = Some(Transition {
            kind: TransitionKind::CrossDissolve,
            duration: Ticks(10),
        });
        let edl = to_edl(
            &sequence(vec![clip(1, 0, 50, 0), incoming], Vec::new()),
            "Dissolve",
            0,
        );
        assert_eq!(
            events(&edl),
            [
                "001  AX       V     C        00:00:00:00 00:00:01:20 00:00:00:00 00:00:01:20",
                "002  AX       V     C        00:00:01:20 00:00:01:20 00:00:01:20 00:00:01:20",
                "002  AX       V     D    010 00:00:07:20 00:00:10:00 00:00:01:20 00:00:04:00",
            ]
        );
    }

    #[test]
    fn a_speed_change_adds_a_motion_line_from_the_source_in() {
        let mut fast = clip(1, 0, 50, 25);
        fast.speed = Speed::new(2, 1).expect("valid speed");
        let edl = to_edl(&sequence(vec![fast], Vec::new()), "Speed", 0);
        assert_eq!(
            events(&edl),
            [
                "001  AX       V     C        00:00:01:00 00:00:03:00 00:00:00:00 00:00:02:00",
                "M2   AX             050.0                00:00:01:00",
            ]
        );
    }
}
