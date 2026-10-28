use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::{Clip, Sequence, SourceId};

/// Which media a collected project keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Collect {
    #[default]
    All,
    /// Drops files no clip uses; sequences, titles and mattes stay.
    UsedOnly,
}

/// A file to copy for a collected project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCopy {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// The project as it reads once its files are copied into `folder`, with the copies to
/// make: every media file, proxy and LUT once, renamed where two share a name.
pub fn collect_files(
    sequence: &Sequence,
    folder: &Path,
    keep: Collect,
) -> (Sequence, Vec<FileCopy>) {
    let mut collected = sequence.clone();
    if keep == Collect::UsedOnly {
        let used: HashSet<SourceId> = sequence.all_clips().map(|clip| clip.source).collect();
        collected.media.retain(|item| {
            item.nest.is_some() || item.generator.is_some() || used.contains(&item.id)
        });
    }
    let mut copier = Copier {
        folder,
        copies: Vec::new(),
        targets: HashMap::new(),
        names: HashSet::new(),
    };
    for item in &mut collected.media {
        if item.nest.is_none() && item.generator.is_none() {
            item.path = copier.copy(&item.path);
        }
        if let Some(proxy) = &mut item.proxy {
            *proxy = copier.copy(proxy);
        }
    }
    let mut luts = |clip: &mut Clip| {
        let color = &mut clip.look.color;
        color
            .input_lut
            .iter_mut()
            .chain(color.look.iter_mut())
            .for_each(|lut| *lut = copier.copy(lut));
    };
    for track in collected.video.iter_mut().chain(&mut collected.audio) {
        track.clips.iter_mut().for_each(&mut luts);
    }
    for nest in collected
        .media
        .iter_mut()
        .filter_map(|item| item.nest.as_mut())
    {
        for track in nest.video.iter_mut().chain(&mut nest.audio) {
            track.clips.iter_mut().for_each(&mut luts);
        }
    }
    (collected, copier.copies)
}

struct Copier<'a> {
    folder: &'a Path,
    copies: Vec<FileCopy>,
    targets: HashMap<PathBuf, PathBuf>,
    /// File names taken in the folder, lowercased since Windows ignores case.
    names: HashSet<String>,
}

impl Copier<'_> {
    fn copy(&mut self, from: &Path) -> PathBuf {
        if let Some(to) = self.targets.get(from) {
            return to.clone();
        }
        let stem = from.file_stem().unwrap_or_default().to_string_lossy();
        let extension = from
            .extension()
            .map(|extension| extension.to_string_lossy());
        let name = (1..)
            .map(|count| {
                let mut name = OsString::from(stem.as_ref());
                if count > 1 {
                    name.push(format!(" ({count})"));
                }
                if let Some(extension) = &extension {
                    name.push(".");
                    name.push(extension.as_ref());
                }
                name
            })
            .find(|name| !self.names.contains(&name.to_string_lossy().to_lowercase()))
            .unwrap_or_default();
        self.names.insert(name.to_string_lossy().to_lowercase());
        let to = self.folder.join(name);
        self.targets.insert(from.to_path_buf(), to.clone());
        self.copies.push(FileCopy {
            from: from.to_path_buf(),
            to: to.clone(),
        });
        to
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ImportMedia, PlaceClip, PlaceMode, Ticks, Timebase, Timeline, VideoFormat};

    #[test]
    fn files_are_copied_once_under_unique_names_and_unused_ones_can_be_left_out() {
        let mut sequence = Sequence::new(Timebase::HZ_90K, VideoFormat::default());
        sequence.audio.resize_with(1, Default::default);
        let mut timeline = Timeline::new(sequence);
        for path in ["/a/take.mov", "/b/Take.mov", "/c/unused.wav", "/a/take.mov"] {
            let import = ImportMedia {
                name: path.into(),
                path: path.into(),
                duration: Ticks(100),
                video: None,
                has_audio: true,
            };
            timeline.apply(import).unwrap();
        }
        let ids: Vec<SourceId> = timeline
            .sequence()
            .media
            .iter()
            .map(|item| item.id)
            .collect();
        for (index, source) in [ids[0], ids[1], ids[3]].into_iter().enumerate() {
            let place = PlaceClip {
                source,
                source_in: Ticks::ZERO,
                duration: Ticks(10),
                at: Ticks(index as i64 * 10),
                video_track: None,
                audio_track: Some(0),
                mode: PlaceMode::Overwrite,
                fill: None,
            };
            timeline.apply(place).unwrap();
        }
        let folder = Path::new("/out");
        let (collected, copies) = collect_files(timeline.sequence(), folder, Collect::UsedOnly);
        let paths: Vec<&Path> = collected
            .media
            .iter()
            .map(|item| item.path.as_path())
            .collect();
        assert_eq!(
            paths,
            [
                Path::new("/out/take.mov"),
                Path::new("/out/Take (2).mov"),
                Path::new("/out/take.mov")
            ]
        );
        assert_eq!(copies.len(), 2);
        let (all, _) = collect_files(timeline.sequence(), folder, Collect::All);
        assert_eq!(all.media[2].path, Path::new("/out/unused.wav"));
    }
}
