use std::collections::HashMap;
use std::path::PathBuf;

use crate::file_url::path_from_url;
use crate::xmeml_markers::XmlMarker;
use crate::xml::{self, Element};
use crate::{
    Alignment, Clip, ClipId, Edit, FrameRate, Gain, LinkId, MediaItem, Nest, Sequence, SourceId,
    Ticks, TimelineError, Track, Transition, TransitionKind, VideoFormat,
};

/// A sequence read from Final Cut Pro XML, as Premiere, Resolve and Final Cut Pro 7
/// write it. Applied, it joins the project as a nested sequence, adding its files to the
/// pool or reusing the ones already there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportXmeml {
    pub name: String,
    rate: FrameRate,
    files: Vec<XmlFile>,
    video: Vec<Vec<XmlClip>>,
    audio: Vec<Vec<XmlClip>>,
    links: usize,
    markers: Vec<XmlMarker>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct XmlFile {
    name: String,
    path: PathBuf,
    rate: FrameRate,
    frames: Option<i64>,
    size: Option<(u32, u32)>,
    video: bool,
    audio: bool,
}

/// Times in frames: `start` and `end` at the sequence rate, `source_in` at the clip's.
#[derive(Debug, Clone, PartialEq, Eq)]
struct XmlClip {
    file: usize,
    rate: FrameRate,
    start: i64,
    end: i64,
    source_in: i64,
    disabled: bool,
    link: Option<usize>,
    gain: Gain,
    transition_in: Option<(TransitionKind, i64, Alignment)>,
    transition_out: Option<(TransitionKind, i64)>,
    markers: Vec<XmlMarker>,
}

/// One child of a `<track>` in document order, with times as written, -1 for unknown.
enum Item {
    Transition {
        start: i64,
        end: i64,
        alignment: String,
        kind: TransitionKind,
    },
    Clip {
        clip: XmlClip,
        out: i64,
    },
}

/// Reads the first sequence in a Final Cut Pro XML document.
pub fn read_xmeml(text: &str) -> Result<ImportXmeml, TimelineError> {
    let root = xml::parse(text)?;
    if root.name != "xmeml" {
        return Err(unreadable("it is not Final Cut Pro XML"));
    }
    let sequence =
        first_named(&root, "sequence").ok_or_else(|| unreadable("it has no sequence"))?;
    let rate = rate_of(sequence)
        .or_else(|| {
            let format = sequence.find(&["media", "video", "format", "samplecharacteristics"]);
            format.and_then(rate_of)
        })
        .ok_or_else(|| unreadable("the sequence has no frame rate"))?;
    let mut reader = Reader {
        rate,
        files: Vec::new(),
        file_keys: HashMap::new(),
        link_keys: HashMap::new(),
    };
    let mut tracks = |kind: &str| -> Result<Vec<Vec<XmlClip>>, TimelineError> {
        let Some(media) = sequence.find(&["media", kind]) else {
            return Ok(Vec::new());
        };
        media
            .children("track")
            .map(|track| reader.track(track, kind == "video"))
            .collect()
    };
    let video = tracks("video")?;
    let audio = tracks("audio")?;
    Ok(ImportXmeml {
        name: sequence
            .text_at(&["name"])
            .filter(|name| !name.is_empty())
            .unwrap_or("Imported Sequence")
            .to_owned(),
        rate,
        files: reader.files,
        video,
        audio,
        links: reader.link_keys.len(),
        markers: XmlMarker::all(sequence),
    })
}

struct Reader {
    rate: FrameRate,
    files: Vec<XmlFile>,
    file_keys: HashMap<String, usize>,
    /// Each linked set by the lowest clip id in it, which every member names.
    link_keys: HashMap<String, usize>,
}

impl Reader {
    fn track(&mut self, track: &Element, video: bool) -> Result<Vec<XmlClip>, TimelineError> {
        let mut items = Vec::new();
        for element in &track.children {
            match element.name.as_str() {
                "transitionitem" => items.push(self.transition(element)),
                "clipitem" => {
                    if let Some(item) = self.clip(element, video)? {
                        items.push(item);
                    }
                }
                _ => {}
            }
        }
        Ok(resolve(items))
    }

    fn transition(&self, element: &Element) -> Item {
        let name = element.text_at(&["effect", "name"]).unwrap_or_default();
        let kind = TransitionKind::VIDEO
            .into_iter()
            .find(|kind| kind.name().eq_ignore_ascii_case(name))
            .unwrap_or(TransitionKind::CrossDissolve);
        Item::Transition {
            start: element.number_at(&["start"]).unwrap_or(-1),
            end: element.number_at(&["end"]).unwrap_or(-1),
            alignment: element
                .text_at(&["alignment"])
                .unwrap_or("center")
                .to_owned(),
            kind,
        }
    }

    /// A clip of a file; clips of generators and nested sequences have none and are
    /// skipped.
    fn clip(&mut self, element: &Element, video: bool) -> Result<Option<Item>, TimelineError> {
        let Some(file) = element.child("file") else {
            return Ok(None);
        };
        let file = self.file(file, video)?;
        let rate = rate_of(element).unwrap_or(self.rate);
        let links: Vec<&str> = element
            .children("link")
            .filter_map(|link| link.text_at(&["linkclipref"]))
            .collect();
        let link = (links.len() > 1)
            .then(|| links.iter().min().map(|key| key.to_string()))
            .flatten()
            .map(|key| {
                let next = self.link_keys.len();
                *self.link_keys.entry(key).or_insert(next)
            });
        let clip = XmlClip {
            file,
            rate,
            start: element.number_at(&["start"]).unwrap_or(-1),
            end: element.number_at(&["end"]).unwrap_or(-1),
            source_in: element.number_at(&["in"]).unwrap_or(0).max(0),
            disabled: element.text_at(&["enabled"]) == Some("FALSE"),
            link,
            gain: level(element),
            transition_in: None,
            transition_out: None,
            markers: XmlMarker::all(element),
        };
        let out = element.number_at(&["out"]).unwrap_or(-1);
        Ok(Some(Item::Clip { clip, out }))
    }

    /// The file a clip uses; the first mention describes it and later ones name its id.
    fn file(&mut self, element: &Element, video: bool) -> Result<usize, TimelineError> {
        let key = element.attribute("id").unwrap_or_default().to_owned();
        let described = element.child("pathurl").is_some() || element.child("name").is_some();
        let index = match self.file_keys.get(&key) {
            Some(&index) if !described || key.is_empty() => index,
            _ if !described => {
                return Err(unreadable(&format!(
                    "file {key} is used before it is described"
                )));
            }
            _ => {
                let name = element.text_at(&["name"]).unwrap_or_default().to_owned();
                let path = element
                    .text_at(&["pathurl"])
                    .map_or_else(|| PathBuf::from(&name), path_from_url);
                let media = element.child("media");
                let size = media
                    .and_then(|media| media.find(&["video", "samplecharacteristics"]))
                    .and_then(|sample| {
                        let width = sample.number_at(&["width"])?;
                        let height = sample.number_at(&["height"])?;
                        Some((u32::try_from(width).ok()?, u32::try_from(height).ok()?))
                    });
                self.files.push(XmlFile {
                    name: path
                        .file_name()
                        .map_or(name, |file| file.to_string_lossy().into_owned()),
                    path,
                    rate: rate_of(element).unwrap_or(self.rate),
                    frames: element
                        .number_at(&["duration"])
                        .filter(|&frames| frames > 0),
                    size,
                    video: media.is_some_and(|media| media.child("video").is_some()),
                    audio: media.is_some_and(|media| media.child("audio").is_some()),
                });
                self.file_keys.insert(key, self.files.len() - 1);
                self.files.len() - 1
            }
        };
        let file = &mut self.files[index];
        if video {
            file.video = true;
        } else {
            file.audio = true;
        }
        Ok(index)
    }
}

/// Fills in the times written as -1 around transitions, which sit at the cut between
/// the clips, and hands each transition to the clip it belongs to.
fn resolve(items: Vec<Item>) -> Vec<XmlClip> {
    let mut clips: Vec<XmlClip> = Vec::new();
    let mut pending: Option<(i64, TransitionKind, i64, Alignment)> = None;
    let mut open_end = false;
    for item in items {
        match item {
            Item::Transition {
                start,
                end,
                alignment,
                kind,
            } => {
                let length = (end - start).max(0);
                let cut = match alignment.as_str() {
                    "start" | "start-black" => start,
                    "end" | "end-black" => end,
                    _ => start + length / 2,
                };
                if alignment == "end-black" {
                    if let Some(last) = clips.last_mut() {
                        last.transition_out = Some((kind, length));
                    }
                } else {
                    if open_end {
                        if let Some(last) = clips.last_mut() {
                            last.end = cut;
                        }
                    }
                    let aligned = match alignment.as_str() {
                        "start" => Alignment::StartAtCut,
                        "end" => Alignment::EndAtCut,
                        _ => Alignment::Center,
                    };
                    pending = Some((cut, kind, length, aligned));
                }
                open_end = false;
            }
            Item::Clip { mut clip, out } => {
                let length = (out - clip.source_in).max(0);
                if let Some((cut, kind, length, alignment)) = pending.take() {
                    if clip.start < 0 {
                        clip.start = cut;
                    }
                    clip.transition_in = Some((kind, length, alignment));
                }
                if clip.start < 0 {
                    clip.start = clips.last().map_or(0, |last| last.end);
                }
                open_end = clip.end < 0;
                if open_end {
                    clip.end = clip.start + length;
                }
                clips.push(clip);
            }
        }
    }
    clips.retain(|clip| clip.end > clip.start);
    clips
}

impl Edit for ImportXmeml {
    fn name(&self) -> &'static str {
        "Import XML"
    }

    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let timebase = sequence.timebase;
        let ticks = |rate: FrameRate, frames: i64| rate.frame_start(frames, timebase);
        let used = |file: usize| {
            self.video
                .iter()
                .chain(&self.audio)
                .flatten()
                .filter(|clip| clip.file == file)
                .map(|clip| {
                    ticks(clip.rate, clip.source_in) + ticks(self.rate, clip.end - clip.start)
                })
                .max()
                .unwrap_or(Ticks(1))
        };
        let mut sources = Vec::new();
        for (index, file) in self.files.iter().enumerate() {
            let existing = sequence.media.iter().find(|item| {
                item.nest.is_none() && item.generator.is_none() && item.path == file.path
            });
            let id = match existing {
                Some(item) => item.id,
                None => {
                    let id = SourceId(sequence.ids.next());
                    let length = file
                        .frames
                        .map_or(Ticks::ZERO, |frames| ticks(file.rate, frames));
                    let (width, height) = file
                        .size
                        .unwrap_or((sequence.format.width, sequence.format.height));
                    sequence.media.push(MediaItem {
                        id,
                        name: file.name.clone(),
                        path: file.path.clone(),
                        duration: length.max(used(index)),
                        video: file.video.then_some(VideoFormat {
                            width,
                            height,
                            frame_rate: file.rate,
                            ..VideoFormat::default()
                        }),
                        has_audio: file.audio,
                        proxy: None,
                        generator: None,
                        nest: None,
                        bin: None,
                        subclip: None,
                        interpreted_rate: None,
                        interpreted_aspect: None,
                        alpha: Default::default(),
                        log: Default::default(),
                        offline: false,
                    });
                    id
                }
            };
            sources.push(id);
        }
        let links: Vec<LinkId> = (0..self.links)
            .map(|_| LinkId(sequence.ids.next()))
            .collect();
        let transition =
            |(kind, frames): (TransitionKind, i64)| Transition::new(kind, ticks(self.rate, frames));
        let mut tracks = |clips: &[Vec<XmlClip>]| -> Vec<Track> {
            clips
                .iter()
                .map(|clips| {
                    let mut track = Track::default();
                    for xml in clips {
                        let start = ticks(self.rate, xml.start);
                        let mut clip = Clip::new(
                            ClipId(sequence.ids.next()),
                            sources[xml.file],
                            start,
                            ticks(self.rate, xml.end) - start,
                            ticks(xml.rate, xml.source_in),
                        );
                        clip.disabled = xml.disabled;
                        clip.gain = xml.gain;
                        clip.link = xml.link.map(|index| links[index]);
                        clip.transition_in =
                            xml.transition_in
                                .map(|(kind, frames, alignment)| Transition {
                                    alignment,
                                    ..transition((kind, frames))
                                });
                        clip.transition_out = xml.transition_out.map(transition);
                        clip.markers = xml
                            .markers
                            .iter()
                            .map(|marker| marker.marker(&mut sequence.ids, xml.rate, timebase))
                            .collect();
                        track.clips.push(clip);
                    }
                    track
                })
                .collect()
        };
        let (video, audio) = (tracks(&self.video), tracks(&self.audio));
        let markers = self
            .markers
            .iter()
            .map(|marker| marker.marker(&mut sequence.ids, self.rate, timebase))
            .collect();
        let nest = Nest {
            video,
            audio,
            markers,
            ..Nest::default()
        };
        let id = SourceId(sequence.ids.next());
        sequence.media.push(MediaItem {
            id,
            name: self.name.clone(),
            path: PathBuf::new(),
            duration: Ticks::ZERO,
            video: None,
            has_audio: false,
            proxy: None,
            generator: None,
            nest: Some(nest),
            bin: None,
            subclip: None,
            interpreted_rate: None,
            interpreted_aspect: None,
            alpha: Default::default(),
            log: Default::default(),
            offline: false,
        });
        sequence.fit_nest(id);
        Ok(())
    }
}

fn unreadable(reason: &str) -> TimelineError {
    TimelineError::UnreadableSequence(reason.to_owned())
}

fn first_named<'a>(element: &'a Element, name: &str) -> Option<&'a Element> {
    if element.name == name {
        return Some(element);
    }
    element
        .children
        .iter()
        .find_map(|child| first_named(child, name))
}

/// The `<rate>` an element carries; NTSC rates run a thousandth slow.
fn rate_of(element: &Element) -> Option<FrameRate> {
    let rate = element.child("rate")?;
    let timebase = u32::try_from(rate.number_at(&["timebase"])?).ok()?;
    match rate.text_at(&["ntsc"]) {
        Some("TRUE") => FrameRate::new(timebase * 1000, 1001),
        _ => FrameRate::new(timebase, 1),
    }
}

/// The Audio Levels filter's amplitude as gain.
fn level(clip: &Element) -> Gain {
    clip.children("filter")
        .filter_map(|filter| filter.child("effect"))
        .find(|effect| effect.text_at(&["effectid"]) == Some("audiolevels"))
        .and_then(|effect| effect.find(&["parameter", "value"]))
        .and_then(|value| value.text.trim().parse::<f32>().ok())
        .map_or(Gain::UNITY, |amplitude| {
            Gain::from_db(20.0 * amplitude.max(1e-6).log10())
        })
}

#[cfg(test)]
#[path = "xmeml_import_tests.rs"]
mod tests;
