use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossbeam::channel::{self, Receiver, Sender};

use crate::editor::Editor;
use crate::same_path::same_path;

/// How long an answer about an unchanged path holds, so files written or removed while
/// the dialog stays open are noticed.
const RECHECK: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Holder {
    File,
    Queue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    Free,
    /// Already on disk, or to be written by a queued export. `free` is the first
    /// numbered name beside it that's neither.
    Taken {
        by: Holder,
        free: Option<PathBuf>,
    },
}

struct Request {
    path: PathBuf,
    queued: Vec<PathBuf>,
    frames: bool,
}

/// Looks on a thread of its own whether an export would replace something, as the
/// output folder may be on a slow disk or network share.
pub(crate) struct TargetCheck {
    requests: Sender<Request>,
    answers: Receiver<(PathBuf, Target)>,
    asked: Option<(PathBuf, Instant)>,
    answer: Option<(PathBuf, Target)>,
}

impl TargetCheck {
    pub(crate) fn new(ctx: egui::Context) -> std::io::Result<Self> {
        let (requests, asked) = channel::unbounded::<Request>();
        let (answer, answers) = channel::unbounded();
        std::thread::Builder::new()
            .name("export target".into())
            .spawn(move || {
                while let Ok(mut request) = asked.recv() {
                    // Only the newest matters while the name is being typed.
                    if let Some(newest) = asked.try_iter().last() {
                        request = newest;
                    }
                    let on_disk = |path: &Path| {
                        path.is_file() || request.frames && first_frame(path).is_file()
                    };
                    let target = check(&request.path, &request.queued, on_disk);
                    if answer.send((request.path, target)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })?;
        Ok(Self {
            requests,
            answers,
            asked: None,
            answer: None,
        })
    }

    /// What exporting to `path`, or to numbered `frames` beside it, would replace, or
    /// `None` until the thread has looked.
    pub(crate) fn target(
        &mut self,
        path: &Path,
        frames: bool,
        queued: impl FnOnce() -> Vec<PathBuf>,
    ) -> Option<&Target> {
        if let Some(newest) = self.answers.try_iter().last() {
            self.answer = Some(newest);
        }
        let stale = self
            .asked
            .as_ref()
            .is_none_or(|(asked, at)| asked != path || at.elapsed() >= RECHECK);
        if stale {
            let request = Request {
                path: path.to_owned(),
                queued: queued(),
                frames,
            };
            // The thread only stops when this is dropped.
            let _ = self.requests.send(request);
            self.asked = Some((path.to_owned(), Instant::now()));
        }
        self.answer
            .as_ref()
            .filter(|(answered, _)| answered == path)
            .map(|(_, target)| target)
    }
}

fn check(path: &Path, queued: &[PathBuf], on_disk: impl Fn(&Path) -> bool) -> Target {
    let holder = |path: &Path| {
        if queued.iter().any(|queued| same_path(queued, path)) {
            Some(Holder::Queue)
        } else if on_disk(path) {
            Some(Holder::File)
        } else {
            None
        }
    };
    match holder(path) {
        None => Target::Free,
        Some(by) => Target::Taken {
            by,
            free: numbered(path).find(|candidate| holder(candidate).is_none()),
        },
    }
}

/// The first image an image-sequence export to `path` writes. Exports of 10,000 frames
/// or more number with more digits, but one that long over an old one is rare.
pub(crate) fn first_frame(path: &Path) -> PathBuf {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path.extension().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!("{stem}0000.{extension}"))
}

/// "Name 2.mp4", "Name 3.mp4" and on beside `path`, counting on from a number its
/// name already ends with.
fn numbered(path: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    let stem = path
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
    let (base, last) = match stem.rsplit_once(' ') {
        Some((base, number))
            if !base.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            match number.parse::<u32>() {
                Ok(last) => (base.to_owned(), last),
                Err(_) => (stem.clone(), 1),
            }
        }
        _ => (stem.clone(), 1),
    };
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    (last.saturating_add(1)..last.saturating_add(1000))
        .map(move |number| path.with_file_name(format!("{base} {number}{extension}")))
}

impl Editor {
    /// Whether exporting to `path` would write over the project file or a file the
    /// project reads from while the export does.
    pub(crate) fn is_project_file(&self, path: &Path) -> bool {
        let media = self.timeline.project().media.iter();
        let sources = media
            .filter(|item| item.generator.is_none() && item.nest.is_none())
            .flat_map(|item| std::iter::once(&item.path).chain(&item.proxy));
        self.project_path
            .iter()
            .chain(sources)
            .any(|used| same_path(used, path))
    }

    /// A file name for what the open sequence makes: the sequence's own name, else the
    /// project's.
    pub(crate) fn output_stem(&self) -> String {
        let project_stem = self
            .project_path
            .as_deref()
            .and_then(Path::file_stem)
            .filter(|_| self.timeline.open_nest().is_none());
        let name = project_stem.map_or_else(
            || self.shown_sequence_name().to_owned(),
            |stem| stem.to_string_lossy().into_owned(),
        );
        // Characters Windows doesn't allow in file names.
        name.replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "-")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_taken_name_offers_the_next_free_number() {
        let on_disk = ["/out/Cut.mp4", "/out/Cut 2.mp4", "/out/Cut 7.mp4"].map(PathBuf::from);
        let queued = [PathBuf::from("/out/Cut 3.mp4")];
        let check = |path: &str| {
            check(Path::new(path), &queued, |path| {
                on_disk.iter().any(|file| file == path)
            })
        };
        let taken = |by, free: &str| Target::Taken {
            by,
            free: Some(PathBuf::from(free)),
        };
        assert_eq!(check("/out/Other.mp4"), Target::Free);
        assert_eq!(check("/out/Cut.mp4"), taken(Holder::File, "/out/Cut 4.mp4"));
        assert_eq!(
            check("/out/Cut 3.mp4"),
            taken(Holder::Queue, "/out/Cut 4.mp4")
        );
        assert_eq!(
            check("/out/Cut 7.mp4"),
            taken(Holder::File, "/out/Cut 8.mp4")
        );
    }
}
