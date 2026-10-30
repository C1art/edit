//! Final Cut Pro 7 XML (xmeml), the timeline interchange Premiere, Resolve and Final Cut
//! read: every track's clips with their files, links, speed, level, transitions and
//! markers.
//! Generated media and nested sequences have no file to point at, so they are left out.

use std::collections::HashMap;
use std::fmt::{Display, Write};
use std::path::PathBuf;
use std::sync::Arc;

use crate::file_url::path_url;
use crate::{
    cut_window, Alignment, Clip, ClipId, FrameRate, Marker, MediaItem, Sequence, Ticks, Track,
    TrackKind, Transition,
};

/// `sequence` as an xmeml document, named `title`.
pub fn to_xmeml(sequence: &Sequence, title: &str) -> String {
    let mut writer = Writer {
        sequence,
        text: String::new(),
        depth: 0,
        files: Vec::new(),
        places: places(sequence),
    };
    writer.document(title);
    writer.text
}

/// Where each exported clip sits, as links name it: its kind, track and position on the
/// track, counting from one.
#[derive(Debug, Clone, Copy)]
struct Place {
    kind: TrackKind,
    track: usize,
    index: usize,
}

fn places(sequence: &Sequence) -> HashMap<ClipId, Place> {
    let mut places = HashMap::new();
    for (track_ref, track) in sequence.tracks() {
        for (index, clip) in exported(sequence, track).enumerate() {
            let place = Place {
                kind: track_ref.kind,
                track: track_ref.index + 1,
                index: index + 1,
            };
            places.insert(clip.id, place);
        }
    }
    places
}

fn exported<'a>(sequence: &'a Sequence, track: &'a Track) -> impl Iterator<Item = &'a Clip> {
    track
        .clips
        .iter()
        .map(Arc::as_ref)
        .filter(|clip| file_of(sequence, clip).is_some())
}

fn file_of<'a>(sequence: &'a Sequence, clip: &Clip) -> Option<&'a MediaItem> {
    sequence
        .media_item(clip.source)
        .filter(|item| item.generator.is_none() && item.nest.is_none())
}

struct Writer<'a> {
    sequence: &'a Sequence,
    text: String,
    depth: usize,
    /// Files written in full so far; later clips of the same file refer to them by id.
    files: Vec<PathBuf>,
    places: HashMap<ClipId, Place>,
}

impl Writer<'_> {
    fn document(&mut self, title: &str) {
        let sequence = self.sequence;
        self.line("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
        self.line("<!DOCTYPE xmeml>");
        self.open("xmeml version=\"4\"");
        self.open("sequence id=\"sequence-1\"");
        self.leaf("name", escape(title));
        self.leaf("duration", self.frames(sequence.duration()));
        self.rate();
        self.markers(&sequence.markers);
        self.open("media");
        self.open("video");
        self.open("format");
        self.open("samplecharacteristics");
        self.rate();
        self.leaf("width", sequence.format.width);
        self.leaf("height", sequence.format.height);
        self.close("samplecharacteristics");
        self.close("format");
        for track in &sequence.video {
            self.track(track, TrackKind::Video);
        }
        self.close("video");
        self.open("audio");
        for track in &sequence.audio {
            self.track(track, TrackKind::Audio);
        }
        self.close("audio");
        self.close("media");
        self.close("sequence");
        self.close("xmeml");
    }

    fn track(&mut self, track: &Track, kind: TrackKind) {
        self.open("track");
        let clips: Vec<&Clip> = exported(self.sequence, track).collect();
        for (index, &clip) in clips.iter().enumerate() {
            let previous = index
                .checked_sub(1)
                .map(|before| clips[before])
                .filter(|previous| previous.end() == clip.start);
            let next = clips
                .get(index + 1)
                .copied()
                .filter(|next| next.start == clip.end());
            let mut start = self.frames(clip.start);
            if let Some(transition) = clip.transition_in {
                let span = match previous {
                    Some(previous) => {
                        // Both clips run on under a transition at their cut.
                        start = -1;
                        let (before, after) = cut_window(previous, clip, transition);
                        let alignment = match transition.alignment {
                            Alignment::Center => "center",
                            Alignment::StartAtCut => "start",
                            Alignment::EndAtCut => "end",
                        };
                        (clip.start - before, clip.start + after, alignment)
                    }
                    None => {
                        let length = transition.duration.min(clip.duration);
                        (clip.start, clip.start + length, "start-black")
                    }
                };
                self.transition(transition, kind, span);
            }
            let end = match next {
                Some(next) if next.transition_in.is_some() => -1,
                _ => self.frames(clip.end()),
            };
            self.clip_item(clip, kind, start, end);
            if let Some(transition) = clip.transition_out.filter(|_| next.is_none()) {
                let length = transition.duration.min(clip.duration);
                self.transition(
                    transition,
                    kind,
                    (clip.end() - length, clip.end(), "end-black"),
                );
            }
        }
        self.leaf("enabled", flag(!track.muted));
        self.leaf("locked", flag(track.locked));
        self.close("track");
    }

    fn transition(&mut self, transition: Transition, kind: TrackKind, span: (Ticks, Ticks, &str)) {
        let (start, end, alignment) = span;
        self.open("transitionitem");
        self.leaf("start", self.frames(start));
        self.leaf("end", self.frames(end));
        self.leaf("alignment", alignment);
        self.rate();
        self.open("effect");
        match kind {
            TrackKind::Video => {
                let name = transition.kind.name();
                self.leaf("name", name);
                self.leaf("effectid", name);
                self.leaf("effecttype", "transition");
                self.leaf("mediatype", "video");
            }
            TrackKind::Audio => {
                self.leaf("name", "Cross Fade (+3dB)");
                self.leaf("effectid", "KGAudioTransCrossFade3dB");
                self.leaf("effecttype", "transition");
                self.leaf("mediatype", "audio");
            }
        }
        self.close("effect");
        self.close("transitionitem");
    }

    fn clip_item(&mut self, clip: &Clip, kind: TrackKind, start: i64, end: i64) {
        let Some(item) = file_of(self.sequence, clip) else {
            return;
        };
        self.open(&format!("clipitem id=\"{}\"", clip_ref(clip.id)));
        self.leaf("name", escape(clip.name.as_deref().unwrap_or(&item.name)));
        self.leaf("enabled", flag(!clip.disabled));
        self.leaf("duration", self.frames(item.duration));
        self.rate();
        self.leaf("start", start);
        self.leaf("end", end);
        let source_in = self.frames(clip.source_in);
        self.leaf("in", source_in);
        let length = self.frames(clip.end()) - self.frames(clip.start);
        self.leaf("out", source_in + length);
        self.file(item);
        self.markers(&clip.markers);
        if kind == TrackKind::Audio {
            self.open("sourcetrack");
            self.leaf("mediatype", "audio");
            self.leaf("trackindex", 1);
            self.close("sourcetrack");
        }
        let speed = if clip.hold {
            Some(0.0)
        } else {
            (!clip.speed.is_normal() || clip.reverse).then(|| clip.speed.percent())
        };
        if let Some(percent) = speed.filter(|_| kind == TrackKind::Video) {
            self.open("filter");
            self.open("effect");
            self.leaf("name", "Time Remap");
            self.leaf("effectid", "timeremap");
            self.leaf("effecttype", "motion");
            self.leaf("mediatype", "video");
            self.parameter("speed", percent);
            self.parameter("reverse", flag(clip.reverse));
            self.close("effect");
            self.close("filter");
        }
        if kind == TrackKind::Audio {
            self.open("filter");
            self.open("effect");
            self.leaf("name", "Audio Levels");
            self.leaf("effectid", "audiolevels");
            self.leaf("effecttype", "audiolevels");
            self.leaf("mediatype", "audio");
            self.parameter("level", clip.gain.amplitude());
            self.close("effect");
            self.close("filter");
        }
        if clip.link.is_some() {
            for id in self.sequence.links_of(&[clip.id]) {
                self.link(id);
            }
        }
        self.close("clipitem");
    }

    /// Markers at their frames; a clip's count from the start of its file.
    fn markers(&mut self, markers: &[Marker]) {
        for marker in markers {
            self.open("marker");
            self.leaf("comment", escape(&marker.comment));
            self.leaf("name", escape(&marker.name));
            self.leaf("in", self.frames(marker.at));
            let out = if marker.duration > Ticks::ZERO {
                self.frames(marker.end())
            } else {
                -1
            };
            self.leaf("out", out);
            self.close("marker");
        }
    }

    fn link(&mut self, id: ClipId) {
        let Some(place) = self.places.get(&id).copied() else {
            return;
        };
        self.open("link");
        self.leaf("linkclipref", clip_ref(id));
        self.leaf("mediatype", media_type(place.kind));
        self.leaf("trackindex", place.track);
        self.leaf("clipindex", place.index);
        self.close("link");
    }

    fn parameter(&mut self, id: &str, value: impl Display) {
        self.open("parameter");
        self.leaf("parameterid", id);
        self.leaf("name", id);
        self.leaf("value", value);
        self.close("parameter");
    }

    fn file(&mut self, item: &MediaItem) {
        if let Some(index) = self.files.iter().position(|path| *path == item.path) {
            self.line(&format!("<file id=\"file-{}\"/>", index + 1));
            return;
        }
        self.files.push(item.path.clone());
        self.open(&format!("file id=\"file-{}\"", self.files.len()));
        let name = item
            .path
            .file_name()
            .map_or_else(|| item.name.clone(), |name| name.to_string_lossy().into());
        self.leaf("name", escape(&name));
        self.leaf("pathurl", path_url(&item.path));
        self.rate();
        self.leaf("duration", self.frames(item.duration));
        self.open("media");
        if let Some(video) = item.video {
            self.open("video");
            self.open("samplecharacteristics");
            self.leaf("width", video.width);
            self.leaf("height", video.height);
            self.close("samplecharacteristics");
            self.close("video");
        }
        if item.has_audio {
            self.open("audio");
            self.leaf("channelcount", 2);
            self.close("audio");
        }
        self.close("media");
        self.close("file");
    }

    /// Everything is counted in frames of the sequence.
    fn rate(&mut self) {
        let rate = self.sequence.format.frame_rate;
        self.open("rate");
        self.leaf("timebase", rate.nominal());
        self.leaf("ntsc", flag(is_ntsc(rate)));
        self.close("rate");
    }

    fn frames(&self, time: Ticks) -> i64 {
        let sequence = self.sequence;
        sequence.format.frame_rate.frame_at(time, sequence.timebase)
    }

    fn open(&mut self, tag: &str) {
        self.line(&format!("<{tag}>"));
        self.depth += 1;
    }

    fn close(&mut self, tag: &str) {
        self.depth -= 1;
        self.line(&format!("</{tag}>"));
    }

    fn leaf(&mut self, tag: &str, value: impl Display) {
        self.line(&format!("<{tag}>{value}</{tag}>"));
    }

    fn line(&mut self, line: &str) {
        let _ = writeln!(self.text, "{:indent$}{line}", "", indent = self.depth * 2);
    }
}

fn clip_ref(id: ClipId) -> String {
    format!("clipitem-{}", id.0)
}

fn media_type(kind: TrackKind) -> &'static str {
    match kind {
        TrackKind::Video => "video",
        TrackKind::Audio => "audio",
    }
}

fn flag(on: bool) -> &'static str {
    if on {
        "TRUE"
    } else {
        "FALSE"
    }
}

/// Rates such as 29.97 count frames against the next whole rate, as NTSC does.
fn is_ntsc(rate: FrameRate) -> bool {
    rate.den() == 1001
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ImportMedia, PlaceClip, PlaceMode, Timebase, Timeline, TransitionKind, VideoFormat,
    };

    #[test]
    fn a_dissolve_sits_between_clips_that_run_on_under_it_and_links_pair_them() {
        let format = VideoFormat::default();
        let mut sequence = Sequence::new(Timebase::new(1, 25).expect("valid"), format);
        sequence.video.resize_with(1, Default::default);
        sequence.audio.resize_with(1, Default::default);
        let mut timeline = Timeline::new(sequence);
        let import = ImportMedia {
            name: "a & b".into(),
            path: "/media/a.mp4".into(),
            duration: Ticks(1000),
            video: Some(format),
            has_audio: true,
            still: false,
            interpreted_rate: None,
            placed: None,
            frame_fit: None,
        };
        timeline.apply(import).unwrap();
        let source = timeline.sequence().media[0].id;
        for (at, source_in) in [(0, 0), (50, 200)] {
            let place = PlaceClip {
                source,
                source_in: Ticks(source_in),
                duration: Ticks(50),
                at: Ticks(at),
                video_track: Some(0),
                audio_track: Some(0),
                mode: PlaceMode::Overwrite,
                fill: None,
            };
            timeline.apply(place).unwrap();
        }
        let mut sequence = timeline.sequence().clone();
        Arc::make_mut(&mut sequence.video[0].clips[1]).transition_in =
            Some(Transition::new(TransitionKind::CrossDissolve, Ticks(10)));
        let [first, second] = [0, 1].map(|index| sequence.video[0].clips[index].id.0);
        let partner = sequence.audio[0].clips[1].id.0;

        let xml: String = to_xmeml(&sequence, "Cut").lines().map(str::trim).collect();
        let head = "<name>a &amp; b</name><enabled>TRUE</enabled><duration>1000</duration>\
            <rate><timebase>25</timebase><ntsc>FALSE</ntsc></rate>";
        let expected = [
            format!("<clipitem id=\"clipitem-{first}\">{head}<start>0</start><end>-1</end>"),
            "<transitionitem><start>45</start><end>55</end><alignment>center</alignment>".into(),
            format!("<clipitem id=\"clipitem-{second}\">{head}<start>-1</start><end>100</end><in>200</in><out>250</out><file id=\"file-1\"/>"),
            format!("<link><linkclipref>clipitem-{partner}</linkclipref><mediatype>audio</mediatype><trackindex>1</trackindex><clipindex>2</clipindex></link>"),
            "<pathurl>file://localhost/media/a.mp4</pathurl>".into(),
        ];
        for part in expected {
            assert!(xml.contains(&part), "missing {part} in {xml}");
        }
        assert_eq!(xml.matches("<file id=\"file-1\">").count(), 1);
    }
}
