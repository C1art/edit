use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::file_url::path_from_url;
use crate::xmeml_markers::XmlMarker;
use crate::xml::{self, Element};
use crate::{
    Alignment, Clip, ClipId, Edit, FrameRate, Gain, LinkId, MediaItem, Nest, Sequence, SourceId,
    Speed, Ticks, TimelineError, Track, Transition, TransitionKind, VideoFormat,
};
use resolve::{resolve, Item};

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
    retime: Option<Retime>,
}

/// A Time Remap filter's constant speed, which writers put on the picture only; `None`
/// speed is a frame hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Retime {
    speed: Option<Speed>,
    reverse: bool,
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
    let mut descriptions = HashMap::new();
    file_descriptions(&root, &mut descriptions);
    let mut reader = Reader {
        rate,
        descriptions,
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

struct Reader<'a> {
    rate: FrameRate,
    /// Every file described in full anywhere in the document, by id, for clips that
    /// name a file first described somewhere this reader doesn't walk, such as inside a
    /// nested sequence or a bin.
    descriptions: HashMap<&'a str, &'a Element>,
    files: Vec<XmlFile>,
    file_keys: HashMap<String, usize>,
    /// Each linked set by the lowest clip id in it, which every member names.
    link_keys: HashMap<String, usize>,
}

impl<'a> Reader<'a> {
    fn track(&mut self, track: &'a Element, video: bool) -> Result<Vec<XmlClip>, TimelineError> {
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
    fn clip(&mut self, element: &'a Element, video: bool) -> Result<Option<Item>, TimelineError> {
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
            retime: retime(element),
        };
        let out = element.number_at(&["out"]).unwrap_or(-1);
        Ok(Some(Item::Clip { clip, out }))
    }

    /// The file a clip uses; the first mention describes it and later ones name its id.
    fn file(&mut self, element: &'a Element, video: bool) -> Result<usize, TimelineError> {
        let key = element.attribute("id").unwrap_or_default().to_owned();
        let element = match self.descriptions.get(key.as_str()) {
            Some(&full) if !describes(element) && !self.file_keys.contains_key(&key) => full,
            _ => element,
        };
        let described = describes(element);
        let index = match self.file_keys.get(&key) {
            // Writers that leave out file ids describe every file in full, so an id-less
            // description is always a new file.
            Some(&index) if !described || !key.is_empty() => index,
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

fn retime(clip: &Element) -> Option<Retime> {
    let effect = clip
        .children("filter")
        .filter_map(|filter| filter.child("effect"))
        .find(|effect| effect.text_at(&["effectid"]) == Some("timeremap"))?;
    let parameter = |id: &str| {
        effect
            .children("parameter")
            .find(|parameter| parameter.text_at(&["parameterid"]) == Some(id))
            .and_then(|parameter| parameter.text_at(&["value"]))
            .map(str::trim)
    };
    let percent: f64 = parameter("speed")?.parse().ok()?;
    // In hundredths of a percent, within the 1% to 10,000% clips allow.
    let points = (percent.abs() * 100.0).round();
    let speed = (points > 0.0)
        .then(|| Speed::from_basis_points(points.clamp(100.0, 1_000_000.0) as u64))
        .flatten();
    Some(Retime {
        speed,
        reverse: parameter("reverse") == Some("TRUE") || percent < 0.0,
    })
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

#[path = "xmeml_import_apply.rs"]
mod apply;
#[path = "xmeml_resolve.rs"]
mod resolve;

#[cfg(test)]
#[path = "xmeml_import_tests.rs"]
mod tests;

fn describes(file: &Element) -> bool {
    file.child("pathurl").is_some() || file.child("name").is_some()
}

/// The first full description of each file id under `element`.
fn file_descriptions<'a>(element: &'a Element, found: &mut HashMap<&'a str, &'a Element>) {
    for child in &element.children {
        if child.name == "file" && describes(child) {
            if let Some(id) = child.attribute("id").filter(|id| !id.is_empty()) {
                found.entry(id).or_insert(child);
            }
        }
        file_descriptions(child, found);
    }
}
