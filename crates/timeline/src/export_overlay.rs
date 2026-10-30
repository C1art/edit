//! Pictures stamped over an export alone, as Premiere's export effects: the sequence
//! timecode and an image watermark, on a track above all the others in the copy of the
//! sequence the export renders.

use std::ops::Range;
use std::sync::Arc;

use crate::edit::generated_item;
use crate::{
    Clip, ClipId, Generator, Param, Sequence, SourceId, Ticks, TimecodeSource, Track, VideoEffect,
};

/// How far in from the frame's edges overlays sit, as a fraction of its height.
const MARGIN: f32 = 0.05;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OverlayPlace {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    #[default]
    BottomCenter,
    BottomRight,
}

impl OverlayPlace {
    pub const ALL: [Self; 6] = [
        Self::TopLeft,
        Self::TopCenter,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomCenter,
        Self::BottomRight,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::TopLeft => "Top Left",
            Self::TopCenter => "Top Center",
            Self::TopRight => "Top Right",
            Self::BottomLeft => "Bottom Left",
            Self::BottomCenter => "Bottom Center",
            Self::BottomRight => "Bottom Right",
        }
    }

    /// -1, 0 or 1 across and down, from the frame's center.
    fn side(self) -> [f32; 2] {
        let across = match self {
            Self::TopLeft | Self::BottomLeft => -1.0,
            Self::TopCenter | Self::BottomCenter => 0.0,
            Self::TopRight | Self::BottomRight => 1.0,
        };
        let down = match self {
            Self::TopLeft | Self::TopCenter | Self::TopRight => -1.0,
            _ => 1.0,
        };
        [across, down]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Watermark {
    /// A still image of the project.
    pub source: SourceId,
    pub place: OverlayPlace,
    /// How wide it shows, as a percentage of the frame's width.
    pub width: f32,
    /// Percent.
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ExportOverlays {
    pub timecode: Option<OverlayPlace>,
    pub watermark: Option<Watermark>,
}

impl ExportOverlays {
    pub fn is_empty(&self) -> bool {
        self.timecode.is_none() && self.watermark.is_none()
    }
}

impl Sequence {
    /// Adds tracks over the others showing `overlays` through `span`.
    pub fn add_export_overlays(&mut self, overlays: ExportOverlays, span: Range<Ticks>) {
        let length = span.end - span.start;
        let watermark = overlays
            .watermark
            .and_then(|watermark| self.watermark_clip(watermark, span.start, length));
        let timecode = overlays
            .timecode
            .map(|place| self.timecode_clip(place, span.start, length));
        // A track each, as they overlap, with the timecode over the watermark.
        for clip in [watermark, timecode].into_iter().flatten() {
            self.video.push(Track {
                clips: vec![Arc::new(clip)],
                ..Track::default()
            });
        }
    }

    fn timecode_clip(&mut self, place: OverlayPlace, start: Ticks, length: Ticks) -> Clip {
        let item = generated_item(self, String::new(), Generator::TransparentVideo);
        let source = item.id;
        Arc::make_mut(&mut self.media).push(item);
        let [across, down] = place.side();
        // The Timecode effect places the text's center in percent of the frame; text
        // about a fifth of the frame wide at this size sits clear of the sides.
        let x = 50.0 + across * 30.0;
        let y = 50.0 + down * (50.0 - 100.0 * MARGIN - 4.0);
        let mut clip = Clip::new(ClipId(self.ids.next()), source, start, length, Ticks::ZERO);
        clip.look.effects.push(VideoEffect::Timecode {
            position: [Param::from_value(x), Param::from_value(y)],
            size: Param::from_value(5.0),
            opacity: Param::from_value(80.0),
            source: TimecodeSource::Sequence,
        });
        clip
    }

    fn watermark_clip(
        &mut self,
        watermark: Watermark,
        start: Ticks,
        length: Ticks,
    ) -> Option<Clip> {
        let item = self.media_item(watermark.source)?;
        let image = item.video?;
        let source_in = item.default_range(self.timebase).start;
        let (frame_width, frame_height) = (self.format.width as f32, self.format.height as f32);
        let (image_width, image_height) = (image.width as f32, image.height as f32);
        if image_width <= 0.0 || image_height <= 0.0 {
            return None;
        }
        let fitted = (frame_width / image_width).min(frame_height / image_height);
        let shown_width = frame_width * watermark.width / 100.0;
        let scale = shown_width / (image_width * fitted);
        let shown = [shown_width, image_height * fitted * scale];
        let frame = [frame_width, frame_height];
        let margin = frame_height * MARGIN;
        let [across, down] = watermark.place.side();
        let offset = [0, 1].map(|axis| {
            let side = [across, down][axis];
            side * (frame[axis] / 2.0 - margin - shown[axis] / 2.0)
        });
        let mut clip = Clip::new(
            ClipId(self.ids.next()),
            watermark.source,
            start,
            length,
            source_in,
        );
        let motion = &mut clip.look.motion;
        motion.scale = Param::from_value(scale * 100.0);
        motion.position = offset.map(Param::from_value);
        clip.look.opacity = Param::from_value(watermark.opacity);
        Some(clip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FrameRate, Timebase, VideoFormat};

    #[test]
    fn watermark_is_sized_to_the_frame_width_and_kept_inside_the_margin() {
        let format = VideoFormat {
            width: 1920,
            height: 1080,
            frame_rate: FrameRate::FPS_25,
            ..VideoFormat::default()
        };
        let mut sequence = Sequence::new(Timebase::HZ_90K, format);
        let mut logo = generated_item(&mut sequence, "logo".into(), Generator::BlackVideo);
        logo.generator = None;
        logo.still = true;
        // A square image fits the frame by its height, 1080 pixels.
        logo.video = Some(VideoFormat {
            width: 500,
            height: 500,
            ..format
        });
        let source = logo.id;
        Arc::make_mut(&mut sequence.media).push(logo);
        let overlays = ExportOverlays {
            timecode: Some(OverlayPlace::TopCenter),
            watermark: Some(Watermark {
                source,
                place: OverlayPlace::BottomRight,
                width: 10.0,
                opacity: 50.0,
            }),
        };
        sequence.add_export_overlays(overlays, Ticks(1_000)..Ticks(5_000));

        let [watermark, timecode] = [0, 1].map(|track| &sequence.video[track].clips[0]);
        assert_eq!(
            (watermark.start, watermark.duration),
            (Ticks(1_000), Ticks(4_000))
        );
        let motion = &watermark.look.motion;
        // 192 pixels wide from 1080: about 17.78 percent.
        assert!((motion.scale.value_at(Ticks::ZERO) - 17.78).abs() < 0.01);
        // Its right and bottom edges 54 pixels (5% of the height) in from the frame's.
        let [x, y] = motion
            .position
            .each_ref()
            .map(|axis| axis.value_at(Ticks::ZERO));
        assert!((x - (960.0 - 54.0 - 96.0)).abs() < 0.05, "{x}");
        assert!((y - (540.0 - 54.0 - 96.0)).abs() < 0.05, "{y}");
        assert!(matches!(
            timecode.look.effects[..],
            [VideoEffect::Timecode {
                source: TimecodeSource::Sequence,
                ..
            }]
        ));
    }
}
