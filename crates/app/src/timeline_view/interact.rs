use egui::{Pos2, Rangef};
use timeline::{
    ClipId, DeltaBounds, LinkedSelection, MarkerOwner, MoveMode, Razor, RazorTarget, Sequence,
    SetClipLine, SetClipVolume, SetTrackAutomation, Ticks, Timeline, TrackKind, TrackRef,
};

use super::gesture::{self, Gesture};
use super::layout::{self, Hit, Row};
use super::markers;
use super::rubber_band::{self, BandHit, BandOwner, Envelope};
use super::scale::{self, TimeScale};
use super::track_select;
use super::transition_grip;
use super::{Drag, TimelineOutput, TimelineView, Tool, SNAP_POINTS};

impl TimelineView {
    pub(super) fn handle_gestures(
        &mut self,
        response: &egui::Response,
        timeline: &mut Timeline,
        scale: TimeScale,
        top: f32,
        playhead: Ticks,
        output: &mut TimelineOutput,
    ) {
        let sequence = timeline.sequence();
        let rows = layout::rows(sequence, &self.heights, top);
        let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
        let frame_time = |x: f32| scale::nearest_frame(scale.time(x), rate, timebase);
        let (shift, command, alt) = response.ctx.input(|input| {
            let modifiers = input.modifiers;
            (modifiers.shift, modifiers.command, modifiers.alt)
        });

        if response.double_clicked() && self.tool.on_timeline() == Tool::Selection {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Hit::Body { clip, .. } = layout::hit(sequence, &rows, &scale, pos) {
                    let marker = sequence
                        .clip(clip)
                        .and_then(|(_, found)| markers::clip_hit(&scale, found, pos.x));
                    match marker {
                        Some(id) => output.edit_marker = Some((MarkerOwner::Clip(clip), id)),
                        None => output.open_clip = Some(clip),
                    }
                    return;
                }
            }
        }
        if response.secondary_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let hit = layout::hit(sequence, &rows, &scale, pos);
                self.context_clip = matches!(hit, Hit::Body { .. } | Hit::Edge { .. });
                self.select_gap(sequence, hit, scale.time(pos.x));
                self.context_gap = self.gap.is_some();
                if let Hit::Body { clip, .. } | Hit::Edge { clip, .. } = hit {
                    if !self.selection.contains(&clip) {
                        self.selection = sequence.with_groups(&[clip]);
                    }
                }
            }
            return;
        }
        if response.clicked() || response.drag_started() {
            self.edit_point = None;
        }
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let hit = layout::hit(sequence, &rows, &scale, pos);
                if self.tool.on_timeline() == Tool::Razor {
                    if let Hit::Body { clip, .. } | Hit::Edge { clip, .. } = hit {
                        let razor = Razor {
                            at: frame_time(pos.x),
                            target: RazorTarget::Clips(vec![clip]),
                        };
                        if let Err(error) = timeline.apply(razor) {
                            output.errors.push(error.to_string());
                        }
                    }
                } else if let Tool::TrackSelect(direction) = self.tool.on_timeline() {
                    let only = shift.then(|| hit_track(hit)).flatten();
                    self.selection =
                        track_select::pick(sequence, frame_time(pos.x), direction, only);
                } else if let Some((owner, grabbed, _)) = self
                    .band_hit(sequence, &rows, &scale, hit, pos)
                    .filter(|_| command && self.tool.on_timeline() == Tool::Selection)
                {
                    toggle_level_key(timeline, owner, grabbed, output);
                } else {
                    self.click(sequence, hit, shift, alt);
                    self.select_gap(sequence, hit, scale.time(pos.x));
                }
            }
            return;
        }

        if response.drag_started() {
            let Some(pos) = response.interact_pointer_pos() else {
                return;
            };
            let origin = response
                .ctx
                .input(|input| input.pointer.press_origin())
                .unwrap_or(pos);
            let hit = layout::hit(sequence, &rows, &scale, origin);
            // Ctrl+Alt rearranges whole linked clips rather than one side of them.
            let links = if alt && !command {
                LinkedSelection::Apart
            } else {
                LinkedSelection::Together
            };
            let unlinked;
            let sequence = match links {
                LinkedSelection::Together => sequence,
                LinkedSelection::Apart => {
                    unlinked = gesture::apart(sequence);
                    &unlinked
                }
            };
            if let Tool::TrackSelect(direction) = self.tool.on_timeline() {
                let only = shift.then(|| hit_track(hit)).flatten();
                self.selection =
                    track_select::pick(sequence, frame_time(origin.x), direction, only);
            } else if let (
                LinkedSelection::Apart,
                Hit::Body { clip, .. } | Hit::Edge { clip, .. },
            ) = (links, hit)
            {
                self.selection = vec![clip];
            }
            let selecting = self.tool.on_timeline() == Tool::Selection;
            let grip = transition_grip::grip_at(sequence, &rows, &scale, origin)
                .filter(|_| selecting && matches!(hit, Hit::Body { .. }));
            let band = self
                .band_hit(sequence, &rows, &scale, hit, origin)
                .filter(|_| selecting);
            let gesture = match (grip, band) {
                (Some(grip), _) => Some(Gesture::TransitionLength(grip)),
                (None, Some((owner, grabbed, band))) => Some(Gesture::Volume {
                    owner,
                    grabbed,
                    band,
                    origin_y: origin.y,
                }),
                (None, None) => self.gesture(sequence, hit),
            };
            self.drag = gesture.map(|gesture| Drag {
                gesture,
                links,
                origin: frame_time(origin.x),
                pending: None,
                preview: None,
                unlinked: matches!(links, LinkedSelection::Apart).then(|| gesture::apart(sequence)),
                bounds: DeltaBounds::default(),
                snapped_to: None,
            });
        }

        let Some(drag) = &mut self.drag else {
            return;
        };
        let sequence = drag.unlinked.as_ref().unwrap_or(sequence);
        if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let mut delta = frame_time(pos.x) - drag.origin;
                drag.snapped_to = None;
                if self.snapping {
                    let moving = drag.gesture.moving_clips(sequence);
                    let targets = scale::snap_targets(sequence, &moving, playhead);
                    let edges = drag.gesture.moving_edges(sequence);
                    delta = scale::snap(delta, &edges, &targets, scale.span(SNAP_POINTS));
                    drag.snapped_to = targets
                        .into_iter()
                        .find(|&target| edges.iter().any(|&edge| edge + delta == target));
                }
                let over = layout::row_at(&rows, pos.y).map(|row| row.track);
                let mode = match (command, alt) {
                    (true, true) => MoveMode::Rearrange,
                    (true, false) => MoveMode::Insert,
                    (false, _) => MoveMode::Overwrite,
                };
                let pending =
                    drag.gesture
                        .edit(sequence, delta, over, pos.y, mode, &mut drag.bounds);
                // The pointer moves far more often than a frame-snapped edit changes.
                if drag.pending.as_ref() != Some(&pending) {
                    drag.preview = pending.preview(sequence).ok();
                    drag.pending = Some(pending);
                }
            }
        }
        if response.drag_stopped() {
            let pending = self
                .drag
                .take()
                .and_then(|drag| Some((drag.pending?, drag.links)));
            if let Some((pending, links)) = pending {
                match pending.apply(timeline, links) {
                    Ok(()) | Err(timeline::TimelineError::NothingToEdit) => {}
                    Err(error) => output.errors.push(error.to_string()),
                }
            }
        }
    }

    /// The level line the pointer is on, what it grabs there and the band the line
    /// moves in: a shown track's automation anywhere along its row, else a clip's own
    /// volume line or the line a video clip shows.
    pub(super) fn band_hit(
        &self,
        sequence: &Sequence,
        rows: &[Row],
        scale: &TimeScale,
        hit: Hit,
        pointer: Pos2,
    ) -> Option<(BandOwner, BandHit, Rangef)> {
        if let Some(row) =
            layout::row_at(rows, pointer.y).filter(|row| self.automation_shown.contains(&row.track))
        {
            let end = scale.time(scale.left + self.width);
            let track = sequence.track(row.track)?;
            let rect = layout::clip_rect(&row, scale, Ticks::ZERO, end).shrink(1.0);
            let grabbed = rubber_band::hit(rect, scale, &Envelope::track(track, end), pointer)?;
            return Some((
                BandOwner::Track(row.track),
                grabbed,
                rubber_band::band(rect),
            ));
        }
        let Hit::Body { clip, track } = hit else {
            return None;
        };
        let row = rows.iter().find(|row| row.track == track)?;
        let (_, clip) = sequence.clip(clip)?;
        let rect = layout::clip_rect(row, scale, clip.start, clip.end()).shrink(1.0);
        let (owner, envelope) = match track.kind {
            TrackKind::Audio => (BandOwner::Clip(clip.id), Envelope::clip(clip)),
            TrackKind::Video if rubber_band::video_line_shown(rect.expand(1.0)) => (
                BandOwner::Look(clip.id, self.clip_line),
                Envelope::look(clip, self.clip_line, &sequence.format),
            ),
            TrackKind::Video => return None,
        };
        let grabbed = rubber_band::hit(rect, scale, &envelope, pointer)?;
        Some((owner, grabbed, rubber_band::band(rect)))
    }

    /// Selects the gap a click on an empty stretch of a track lands in.
    fn select_gap(&mut self, sequence: &Sequence, hit: Hit, at: Ticks) {
        self.gap = match hit {
            Hit::Empty { track: Some(track) } => sequence
                .track(track)
                .and_then(|found| found.gap_at(at))
                .map(|_| (track, at)),
            _ => None,
        };
    }

    /// Alt picks the clip alone, without its group or linked partners.
    fn click(&mut self, sequence: &Sequence, hit: Hit, shift: bool, alt: bool) {
        let clip = match hit {
            Hit::Body { clip, .. } | Hit::Edge { clip, .. } => clip,
            Hit::Empty { .. } => {
                if !shift {
                    self.selection.clear();
                }
                return;
            }
        };
        let group = if alt {
            vec![clip]
        } else {
            sequence.with_groups(&[clip])
        };
        if shift {
            if self.selection.contains(&clip) {
                self.selection.retain(|id| !group.contains(id));
            } else {
                self.selection.extend(group);
            }
        } else {
            self.selection = group;
        }
    }

    fn gesture(&mut self, sequence: &Sequence, hit: Hit) -> Option<Gesture> {
        let grabbed = |clip: ClipId, selection: &mut Vec<ClipId>| {
            if !selection.contains(&clip) {
                *selection = sequence.with_groups(&[clip]);
            }
            selection.clone()
        };
        match (self.tool.on_timeline(), hit) {
            (Tool::TrackSelect(_), hit) => {
                let (first, _) = self.selection.first().and_then(|&id| sequence.clip(id))?;
                let from = hit_track(hit).unwrap_or(first);
                Some(Gesture::Move {
                    clips: self.selection.clone(),
                    from,
                })
            }
            (Tool::Razor, _) | (_, Hit::Empty { .. }) => None,
            (Tool::Selection | Tool::Ripple, Hit::Edge { clip, edge, .. }) => {
                self.selection = sequence.with_links(&[clip]);
                let mode = if self.tool.on_timeline() == Tool::Ripple {
                    timeline::TrimMode::Ripple
                } else {
                    timeline::TrimMode::Normal
                };
                Some(Gesture::Trim {
                    clips: vec![clip],
                    edge,
                    mode,
                })
            }
            (Tool::RateStretch, Hit::Edge { clip, edge, .. }) => {
                self.selection = sequence.with_links(&[clip]);
                Some(Gesture::Stretch {
                    clips: vec![clip],
                    edge,
                })
            }
            (Tool::RateStretch, Hit::Body { .. }) => None,
            (Tool::Roll, Hit::Edge { clip, at, .. }) => {
                let mut tracks: Vec<TrackRef> = sequence
                    .with_links(&[clip])
                    .into_iter()
                    .filter_map(|id| sequence.clip(id))
                    .filter(|(_, clip)| clip.start == at || clip.end() == at)
                    .map(|(track, _)| track)
                    .collect();
                tracks.dedup();
                Some(Gesture::Roll { at, tracks })
            }
            (Tool::Slip, Hit::Body { clip, .. }) => Some(Gesture::Slip {
                clips: grabbed(clip, &mut self.selection),
            }),
            (Tool::Slide, Hit::Body { clip, .. }) => Some(Gesture::Slide {
                clips: grabbed(clip, &mut self.selection),
            }),
            (_, Hit::Body { clip, track } | Hit::Edge { clip, track, .. }) => Some(Gesture::Move {
                clips: grabbed(clip, &mut self.selection),
                from: track,
            }),
        }
    }
}

fn hit_track(hit: Hit) -> Option<TrackRef> {
    match hit {
        Hit::Body { track, .. } | Hit::Edge { track, .. } => Some(track),
        Hit::Empty { track } => track,
    }
}

/// Adds a keyframe where the line was clicked, or removes the one clicked.
fn toggle_level_key(
    timeline: &mut Timeline,
    owner: BandOwner,
    grabbed: BandHit,
    output: &mut TimelineOutput,
) {
    let sequence = timeline.sequence();
    let Some(envelope) = Envelope::of(sequence, owner, sequence.duration()) else {
        return;
    };
    let mut param = envelope.param().clone();
    let time = match grabbed {
        BandHit::Key(index) => param.keys.get(index).map(|key| key.time),
        BandHit::Line(at) => Some(at),
    };
    let Some(time) = time else {
        return;
    };
    param.toggle_key(time);
    let applied = match owner {
        BandOwner::Clip(clip) => timeline.apply(SetClipVolume {
            clip,
            volume: param,
        }),
        BandOwner::Look(clip, line) => timeline.apply(SetClipLine { clip, line, param }),
        BandOwner::Track(track) => timeline.apply(SetTrackAutomation {
            track,
            automation: param,
        }),
    };
    if let Err(error) = applied {
        output.errors.push(error.to_string());
    }
}
