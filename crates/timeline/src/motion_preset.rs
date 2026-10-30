//! Ready-made animations, set as keyframes on a clip's own properties so they can be
//! adjusted afterwards like any others.

use crate::{Clip, FrameRate, Keyframe, Param, Ticks, Timebase, VideoEffect};

/// How much a zoom enlarges the picture over its clip.
const ZOOM: f32 = 1.2;
/// How much a pan enlarges the picture, enough to keep the frame covered as it travels.
const PAN_ZOOM: f32 = 1.12;
/// How far a pan travels either side of where the picture was, as a share of the
/// frame's width.
const PAN_TRAVEL: f32 = 0.05;
/// The blurriness a blur in starts from and a blur out ends at.
const BLUR: f32 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionPreset {
    ZoomIn,
    ZoomOut,
    PanLeft,
    PanRight,
    FadeIn,
    FadeOut,
    BlurIn,
    BlurOut,
}

/// The sequence frames keyframes land on, and how wide they are.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Frames {
    pub rate: FrameRate,
    pub timebase: Timebase,
    pub width: u32,
}

/// Source times of a clip's first and last frames, and of where a brief animation at
/// either end finishes or begins.
struct Span {
    first: Ticks,
    last: Ticks,
    opened: Ticks,
    closing: Ticks,
}

impl MotionPreset {
    pub const ALL: [Self; 8] = [
        Self::ZoomIn,
        Self::ZoomOut,
        Self::PanLeft,
        Self::PanRight,
        Self::FadeIn,
        Self::FadeOut,
        Self::BlurIn,
        Self::BlurOut,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::ZoomIn => "Zoom In",
            Self::ZoomOut => "Zoom Out",
            Self::PanLeft => "Pan Left",
            Self::PanRight => "Pan Right",
            Self::FadeIn => "Fade In",
            Self::FadeOut => "Fade Out",
            Self::BlurIn => "Blur In",
            Self::BlurOut => "Blur Out",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::ZoomIn => "Scale grows a fifth over the whole clip",
            Self::ZoomOut => "Scale shrinks back by a fifth over the whole clip",
            Self::PanLeft => "The picture, enlarged a little, travels left over the clip",
            Self::PanRight => "The picture, enlarged a little, travels right over the clip",
            Self::FadeIn => "Opacity rises from nothing over the first half second",
            Self::FadeOut => "Opacity falls to nothing over the last half second",
            Self::BlurIn => "A Gaussian Blur clears over the first half second",
            Self::BlurOut => "A Gaussian Blur gathers over the last half second",
        }
    }

    pub(crate) fn apply(self, clip: &mut Clip, frames: Frames) {
        let Some(span) = Span::of(clip, frames) else {
            return;
        };
        let motion = &mut clip.look.motion;
        match self {
            Self::ZoomIn | Self::ZoomOut => {
                let uniform = motion.uniform_scale;
                let scales = [&mut motion.scale, &mut motion.scale_width];
                for scale in scales.into_iter().take(if uniform { 1 } else { 2 }) {
                    let set = scale.value_at(span.first);
                    let ends = if self == Self::ZoomIn {
                        (set, set * ZOOM)
                    } else {
                        (set * ZOOM, set)
                    };
                    ramp(scale, (span.first, ends.0), (span.last, ends.1));
                }
            }
            Self::PanLeft | Self::PanRight => {
                for scale in [&mut motion.scale, &mut motion.scale_width] {
                    if !scale.is_animated() {
                        *scale = Param::from_value(scale.value_at(span.first) * PAN_ZOOM);
                    }
                }
                let [x, _] = &mut motion.position;
                let set = x.value_at(span.first);
                let travel = frames.width as f32 * PAN_TRAVEL;
                let toward = if self == Self::PanLeft { -1.0 } else { 1.0 };
                let ends = (set - toward * travel, set + toward * travel);
                ramp(x, (span.first, ends.0), (span.last, ends.1));
            }
            Self::FadeIn => {
                let opacity = &mut clip.look.opacity;
                let full = opacity.value_at(span.opened);
                ramp(opacity, (span.first, 0.0), (span.opened, full));
            }
            Self::FadeOut => {
                let opacity = &mut clip.look.opacity;
                let full = opacity.value_at(span.closing);
                ramp(opacity, (span.closing, full), (span.last, 0.0));
            }
            Self::BlurIn | Self::BlurOut => {
                let mut blurriness = Param::from_value(0.0);
                if self == Self::BlurIn {
                    ramp(&mut blurriness, (span.first, BLUR), (span.opened, 0.0));
                } else {
                    ramp(&mut blurriness, (span.closing, 0.0), (span.last, BLUR));
                }
                clip.look
                    .effects
                    .push(VideoEffect::GaussianBlur { blurriness });
            }
        }
    }
}

impl Span {
    /// `None` for a clip of a single frame, which has nothing to animate across.
    fn of(clip: &Clip, Frames { rate, timebase, .. }: Frames) -> Option<Self> {
        let frame_at = |position: Ticks| rate.frame_at(position, timebase);
        let frame_start = |frame: i64| rate.frame_start(frame, timebase).max(clip.start);
        let (first, last) = (frame_at(clip.start), frame_at(clip.end() - Ticks(1)));
        if last <= first {
            return None;
        }
        let half_second =
            Timebase::new(1, 2).map_or(0, |half| frame_at(Ticks(1).rescale(half, timebase)).max(1));
        let brief = half_second.min((last - first) / 2).max(1);
        let opened = frame_start(first + brief);
        let closing = frame_start(last - brief);
        let [first, last] = [first, last].map(frame_start);
        Some(Self {
            first: clip.source_time(first),
            last: clip.source_time(last),
            opened: clip.source_time(opened),
            closing: clip.source_time(closing),
        })
    }
}

/// Keys `param` from one value to another, replacing the keyframes between them.
fn ramp(param: &mut Param, (from, start): (Ticks, f32), (to, end): (Ticks, f32)) {
    let between = from.min(to)..=from.max(to);
    param.keys.retain(|key| !between.contains(&key.time));
    let hundredths = |value: f32| (value * 100.0).round() as i32;
    param.keys.push(Keyframe::new(from, hundredths(start)));
    param.keys.push(Keyframe::new(to, hundredths(end)));
    param.keys.sort_by_key(|key| key.time);
}
