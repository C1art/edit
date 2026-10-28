use egui::{Pos2, Rect, Vec2};
use timeline::{Clip, Look, Sequence, Ticks, VideoEffect};

/// Where the clip's picture lands on screen: its center and the screen vectors along
/// one pixel of its width and height, as fitted to the sequence frame.
pub(super) struct PictureFrame {
    pub(super) shown: Rect,
    pub(super) center: Pos2,
    pub(super) x: Vec2,
    pub(super) y: Vec2,
    /// The picture fitted to the sequence frame, in sequence pixels.
    pub(super) fitted: Vec2,
}

impl PictureFrame {
    pub(super) fn new(
        sequence: &Sequence,
        clip: &Clip,
        look: &Look,
        time: Ticks,
        area: Rect,
    ) -> Option<Self> {
        let format = sequence.format;
        let canvas = Vec2::new(format.width as f32, format.height as f32);
        let source = sequence
            .media_item(clip.source)?
            .video
            .map_or(canvas, |video| {
                Vec2::new(video.width as f32, video.height as f32)
            });
        let fit = (canvas.x / source.x).min(canvas.y / source.y);
        let to_screen = (area.width() / canvas.x).min(area.height() / canvas.y);
        let motion = &look.motion;
        let scale = motion.scale.value_at(time) / 100.0 * to_screen;
        let (sin, cos) = motion.rotation.value_at(time).to_radians().sin_cos();
        // Flipping twice leaves the picture as it was.
        let flip = |effect: VideoEffect| {
            let flips = look.effects.iter().filter(|&used| *used == effect).count();
            if flips % 2 == 1 {
                -1.0
            } else {
                1.0
            }
        };
        let position = Vec2::new(
            motion.position[0].value_at(time),
            motion.position[1].value_at(time),
        );
        Some(Self {
            shown: Rect::from_center_size(area.center(), canvas * to_screen),
            center: area.center() + position * to_screen,
            x: Vec2::new(cos, sin) * scale * flip(VideoEffect::HorizontalFlip),
            y: Vec2::new(-sin, cos) * scale * flip(VideoEffect::VerticalFlip),
            fitted: source * fit,
        })
    }

    pub(super) fn to_screen(&self, picture: Vec2) -> Pos2 {
        self.center + self.x * picture.x + self.y * picture.y
    }

    /// The axes stay perpendicular, so projecting onto each inverts `to_screen`.
    pub(super) fn to_picture(&self, screen: Pos2) -> Vec2 {
        let offset = screen - self.center;
        Vec2::new(
            offset.dot(self.x) / self.x.length_sq().max(f32::EPSILON),
            offset.dot(self.y) / self.y.length_sq().max(f32::EPSILON),
        )
    }
}
