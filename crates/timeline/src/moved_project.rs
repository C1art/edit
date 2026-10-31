use std::path::{Component, Path, PathBuf};

use crate::collect::map_files;
use crate::Sequence;

/// Finds the files of a project moved from `saved_in` to `now_in` together with its media:
/// a file that `exists` no longer finds is looked for at the same place relative to the
/// project. Files found where they were, or found nowhere, keep their paths.
pub fn follow_moved_project(
    sequence: &mut Sequence,
    saved_in: &Path,
    now_in: &Path,
    exists: impl Fn(&Path) -> bool,
) {
    if saved_in == now_in {
        return;
    }
    map_files(sequence, |path| {
        if exists(path) {
            return path.to_owned();
        }
        relative_path(path, saved_in)
            .map(|relative| without_parent_steps(&now_in.join(relative)))
            .filter(|moved| exists(moved))
            .unwrap_or_else(|| path.to_owned())
    });
}

/// `path` as reached from `base`, stepping up out of `base` as far as needed; None when
/// the two don't share a drive or root.
fn relative_path(path: &Path, base: &Path) -> Option<PathBuf> {
    let mut path_parts = path.components().peekable();
    let mut base_parts = base.components().peekable();
    let mut shared = 0;
    while let (Some(path_part), Some(base_part)) = (path_parts.peek(), base_parts.peek()) {
        if !same_component(*path_part, *base_part) {
            break;
        }
        path_parts.next();
        base_parts.next();
        shared += 1;
    }
    if shared == 0 {
        return None;
    }
    let steps_up = base_parts.map(|_| Component::ParentDir);
    Some(steps_up.chain(path_parts).collect())
}

#[cfg(windows)]
fn same_component(a: Component, b: Component) -> bool {
    // Windows ignores case in drive letters and names.
    a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
}

#[cfg(not(windows))]
fn same_component(a: Component, b: Component) -> bool {
    a == b
}

fn without_parent_steps(path: &Path) -> PathBuf {
    let mut resolved = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => {}
            part => resolved.push(part),
        }
    }
    resolved
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::{ImportMedia, Ticks, Timebase, Timeline, VideoFormat};

    #[test]
    fn a_moved_project_finds_its_media_beside_and_below_it_again() {
        let mut timeline = Timeline::new(Sequence::new(Timebase::HZ_90K, VideoFormat::default()));
        for path in [
            "/old/project/footage/a.mov",
            "/old/music/b.wav",
            "/elsewhere/c.mov",
            "/old/project/gone.mov",
        ] {
            let import = ImportMedia {
                name: path.into(),
                path: path.into(),
                duration: Ticks(100),
                video: None,
                has_audio: true,
                still: false,
                interpreted_rate: None,
                placed: None,
                frame_fit: None,
                timecode: None,
            };
            timeline.apply(import).unwrap();
        }
        let mut sequence = timeline.sequence().clone();
        let present: HashSet<&Path> = [
            "/new/disk/project/footage/a.mov",
            "/new/disk/music/b.wav",
            "/elsewhere/c.mov",
            "/new/disk/elsewhere/c.mov",
        ]
        .into_iter()
        .map(Path::new)
        .collect();
        follow_moved_project(
            &mut sequence,
            Path::new("/old/project"),
            Path::new("/new/disk/project"),
            |path| present.contains(path),
        );
        let paths: Vec<&Path> = sequence
            .media
            .iter()
            .map(|item| item.path.as_path())
            .collect();
        assert_eq!(
            paths,
            [
                "/new/disk/project/footage/a.mov",
                "/new/disk/music/b.wav",
                "/elsewhere/c.mov",
                "/old/project/gone.mov",
            ]
            .map(Path::new)
        );
    }
}
