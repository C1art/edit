use super::XmlClip;
use crate::{Alignment, TransitionKind};

/// One child of a `<track>` in document order, with times as written, -1 for unknown.
pub(super) enum Item {
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

/// Fills in the times written as -1 around transitions, which sit at the cut between
/// the clips, and hands each transition to the clip it belongs to.
pub(super) fn resolve(items: Vec<Item>) -> Vec<XmlClip> {
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
