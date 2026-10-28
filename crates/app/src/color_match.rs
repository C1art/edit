use std::sync::Arc;
use std::time::Duration;

use playback::{auto_correction, clip_ungraded, matched_correction, BasicCorrection, ToneJob};
use timeline::{ClipId, ColorCorrection, SetClipLook, Ticks};

use crate::editor::Editor;

/// What Lumetri's tone analysis sets a clip's Basic Correction for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToneGoal {
    /// A neutral, well exposed picture of its own.
    Auto,
    /// The look of the Comparison View's reference frame.
    Match,
}

/// A clip's picture, and for matching the reference's, being read on a worker.
pub(crate) struct ToneAnalysis {
    clip: ClipId,
    goal: ToneGoal,
    /// The source time the correction is set at, keyframing it where it is animated.
    at: Ticks,
    job: ToneJob,
}

impl Editor {
    pub(crate) fn analyze_tone(&mut self, clip: ClipId, goal: ToneGoal, at: Ticks) {
        if self.tone_analysis.is_some() {
            return;
        }
        let Some(render_state) = &self.render_state else {
            self.notice("Color analysis needs the GPU renderer");
            return;
        };
        let sequence = self.timeline.sequence();
        let Some((_, found)) = sequence.clip(clip) else {
            return;
        };
        let current_time = self.playhead().clamp(found.start, found.end() - Ticks(1));
        let Some(ungraded) = clip_ungraded(sequence, clip) else {
            return;
        };
        let mut pictures = vec![(ungraded, current_time)];
        if goal == ToneGoal::Match {
            let Some(comparison) = &self.comparison else {
                self.notice("Open the Comparison View to choose a reference frame");
                return;
            };
            pictures.push((Arc::new(sequence.clone()), comparison.reference_time()));
        }
        let (device, queue) = (render_state.device.clone(), render_state.queue.clone());
        match ToneJob::start(pictures, device, queue) {
            Ok(job) => {
                self.tone_analysis = Some(ToneAnalysis {
                    clip,
                    goal,
                    at,
                    job,
                })
            }
            Err(error) => self.notice(format!("Cannot analyze color: {error}")),
        }
    }

    pub(crate) fn analyzing_tone(&self, clip: ClipId) -> bool {
        self.tone_analysis
            .as_ref()
            .is_some_and(|analysis| analysis.clip == clip)
    }

    /// Sets the clip's Basic Correction once its pictures have been read.
    pub(crate) fn poll_tone_analysis(&mut self, ctx: &egui::Context) {
        let Some(analysis) = &self.tone_analysis else {
            return;
        };
        let Some(outcome) = analysis.job.poll() else {
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        };
        let (clip, goal, at) = (analysis.clip, analysis.goal, analysis.at);
        self.tone_analysis = None;
        let stats = match outcome {
            Ok(stats) => stats,
            Err(error) => {
                self.notice(format!("Cannot analyze color: {error}"));
                return;
            }
        };
        let correction = match (goal, stats.as_slice()) {
            (ToneGoal::Auto, [Some(current)]) => auto_correction(current),
            (ToneGoal::Match, [Some(current), Some(reference)]) => {
                matched_correction(current, reference)
            }
            _ => {
                self.notice("There is no picture to analyze there");
                return;
            }
        };
        let Some((_, found)) = self.timeline.sequence().clip(clip) else {
            return;
        };
        let mut look = found.look.clone();
        set_basic(&mut look.color, &correction, at);
        self.apply(SetClipLook { clip, look });
    }
}

fn set_basic(color: &mut ColorCorrection, correction: &BasicCorrection, at: Ticks) {
    let values = [
        (&mut color.temperature, correction.temperature),
        (&mut color.tint, correction.tint),
        (&mut color.exposure, correction.exposure),
        (&mut color.contrast, correction.contrast),
        (&mut color.whites, correction.whites),
        (&mut color.blacks, correction.blacks),
        (&mut color.saturation, correction.saturation),
    ];
    for (param, value) in values {
        param.set_at(at, value);
    }
}
