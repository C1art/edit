use timeline::{ClipId, Razor, RazorTarget, Sequence, Ticks, Timeline, TrackRef};

use super::gesture::Gesture;
use super::layout::{self, Hit};
use super::scale::{self, TimeScale};
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
        let rows = layout::rows(sequence, top);
        let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
        let frame_time = |x: f32| scale::nearest_frame(scale.time(x), rate, timebase);
        let shift = response.ctx.input(|input| input.modifiers.shift);

        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let hit = layout::hit(sequence, &rows, &scale, pos);
                if self.tool == Tool::Razor {
                    if let Hit::Body { clip, .. } | Hit::Edge { clip, .. } = hit {
                        let razor = Razor {
                            at: frame_time(pos.x),
                            target: RazorTarget::Clips(vec![clip]),
                        };
                        if let Err(error) = timeline.apply(razor) {
                            output.errors.push(error.to_string());
                        }
                    }
                } else {
                    self.click(sequence, hit, shift);
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
            self.drag = self.gesture(sequence, hit).map(|gesture| Drag {
                gesture,
                origin: frame_time(origin.x),
                pending: None,
                preview: None,
            });
        }

        let Some(drag) = &mut self.drag else {
            return;
        };
        if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let mut delta = frame_time(pos.x) - drag.origin;
                if self.snapping {
                    let moving = drag.gesture.moving_clips(sequence);
                    let mut targets: Vec<Ticks> = sequence
                        .tracks()
                        .flat_map(|(_, track)| &track.clips)
                        .filter(|clip| !moving.contains(&clip.id))
                        .flat_map(|clip| [clip.start, clip.end()])
                        .collect();
                    targets.extend(sequence.markers.iter().map(|marker| marker.at));
                    targets.push(playhead);
                    targets.push(Ticks::ZERO);
                    let edges = drag.gesture.moving_edges(sequence);
                    delta = scale::snap(delta, &edges, &targets, scale.span(SNAP_POINTS));
                }
                let over = layout::row_at(&rows, pos.y).map(|row| row.track);
                let pending = drag.gesture.edit(sequence, delta, over);
                drag.preview = pending.preview(sequence).ok();
                drag.pending = Some(pending);
            }
        }
        if response.drag_stopped() {
            if let Some(pending) = self.drag.take().and_then(|drag| drag.pending) {
                match pending.apply(timeline) {
                    Ok(()) | Err(timeline::TimelineError::NothingToEdit) => {}
                    Err(error) => output.errors.push(error.to_string()),
                }
            }
        }
    }

    fn click(&mut self, sequence: &Sequence, hit: Hit, shift: bool) {
        let clip = match hit {
            Hit::Body { clip, .. } | Hit::Edge { clip, .. } => clip,
            Hit::Empty { .. } => {
                if !shift {
                    self.selection.clear();
                }
                return;
            }
        };
        let group = sequence.with_links(&[clip]);
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
                *selection = sequence.with_links(&[clip]);
            }
            selection.clone()
        };
        match (self.tool, hit) {
            (Tool::Razor, _) | (_, Hit::Empty { .. }) => None,
            (Tool::Selection | Tool::Ripple, Hit::Edge { clip, edge, .. }) => {
                self.selection = sequence.with_links(&[clip]);
                let mode = if self.tool == Tool::Ripple {
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
