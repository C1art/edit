use egui::{Pos2, Rect, Vec2};
use timeline::{Clip, Look, Sequence, Ticks, VideoEffect};

/// Where the clip's picture lands on screen: its center and the screen vectors along
/// one pixel of its width and height, as fitted to the sequence frame.
pub(super) struct PictureFrame {
    pub(super) shown: Rect,
    pub(super) center: Pos2,
    /// Where the picture's anchor point lands, which it scales and turns about.
    pub(super) anchor: Pos2,
    pub(super) x: Vec2,
    pub(super) y: Vec2,
    /// -1 along each axis the picture is flipped on, 1 along the others.
    pub(super) flip: Vec2,
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
            .played_format()
            .map_or(canvas, |video| {
                Vec2::new(video.width as f32, video.height as f32)
            });
        let fit = (canvas.x / source.x).min(canvas.y / source.y);
        let to_screen = (area.width() / canvas.x).min(area.height() / canvas.y);
        let motion = &look.motion;
        let [width, height] = motion
            .scale_at(time)
            .map(|percent| percent / 100.0 * to_screen);
        let (sin, cos) = motion.rotation.value_at(time).to_radians().sin_cos();
        // Flipping twice leaves the picture as it was.
        let flipped = |effect: VideoEffect| {
            let flips = look.effects.iter().filter(|&used| *used == effect).count();
            if flips % 2 == 1 {
                -1.0
            } else {
                1.0
            }
        };
        let flip = Vec2::new(
            flipped(VideoEffect::HorizontalFlip),
            flipped(VideoEffect::VerticalFlip),
        );
        let position = Vec2::new(
            motion.position[0].value_at(time),
            motion.position[1].value_at(time),
        );
        let anchor_point = Vec2::new(
            motion.anchor[0].value_at(time),
            motion.anchor[1].value_at(time),
        );
        let anchor = area.center() + position * to_screen;
        let (along_x, along_y) = (Vec2::new(cos, sin) * width, Vec2::new(-sin, cos) * height);
        // The anchor sits in the picture as it was before flipping.
        let center = anchor - along_x * anchor_point.x - along_y * anchor_point.y;
        Some(Self {
            shown: Rect::from_center_size(area.center(), canvas * to_screen),
            center,
            anchor,
            x: along_x * flip.x,
            y: along_y * flip.y,
            flip,
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
