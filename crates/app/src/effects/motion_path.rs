//! The path an animated position takes, drawn over the program picture as Premiere does
//! with Motion selected: a dot for each frame, closer together where the picture moves
//! slower, and a square at each keyframe that can be dragged to move the picture there.

use egui::{Color32, Pos2, Rect, Shape, Stroke, Vec2};
use timeline::{Clip, FrameRate, Motion, Ticks, Timebase};

const PATH: Color32 = Color32::from_rgb(220, 220, 220);
const KEY_SIZE: f32 = 7.0;
/// How near, in points, the pointer must be to grab a keyframe.
const KEY_REACH: f32 = 6.0;
/// Long clips get a dot every so many frames, so the path stays cheap to draw.
const MOST_DOTS: i64 = 600;

pub(super) struct MotionPath {
    dots: Vec<Pos2>,
    /// The source time of each position keyframe in the clip, and where it lands.
    keys: Vec<(Ticks, Pos2)>,
}

impl MotionPath {
    /// The path of the anchor point over `clip`, from the program picture's `center` at
    /// `per_pixel` screen points to a sequence pixel; `None` while it holds still.
    pub(super) fn of(
        clip: &Clip,
        motion: &Motion,
        (rate, timebase): (FrameRate, Timebase),
        center: Pos2,
        per_pixel: f32,
    ) -> Option<Self> {
        let [x, y] = &motion.position;
        if !x.is_animated() && !y.is_animated() {
            return None;
        }
        let at = |time| center + Vec2::new(x.value_at(time), y.value_at(time)) * per_pixel;
        let (first, last) = (
            rate.frame_at(clip.start, timebase),
            rate.frame_at(clip.end() - Ticks(1), timebase),
        );
        let step = usize::try_from(((last - first) / MOST_DOTS).max(1)).unwrap_or(1);
        let dots = (first..=last)
            .step_by(step)
            .map(|frame| {
                let position = rate.frame_start(frame, timebase).max(clip.start);
                at(clip.source_time(position))
            })
            .collect();
        let mut times: Vec<Ticks> = [x, y]
            .into_iter()
            .flat_map(|param| param.keys.iter().map(|key| key.time))
            .filter(|&key| (clip.start..clip.end()).contains(&clip.position_of(key)))
            .collect();
        times.sort_unstable();
        times.dedup();
        let keys = times.into_iter().map(|time| (time, at(time))).collect();
        Some(Self { dots, keys })
    }

    pub(super) fn bounds(&self) -> Rect {
        let points = self.dots.iter().chain(self.keys.iter().map(|(_, at)| at));
        Rect::from_points(&points.copied().collect::<Vec<_>>()).expand(KEY_SIZE)
    }

    /// The keyframe under `pointer`, the one drawn last winning.
    pub(super) fn key_at(&self, pointer: Pos2) -> Option<Ticks> {
        self.keys
            .iter()
            .rev()
            .find(|(_, at)| at.distance(pointer) <= KEY_REACH)
            .map(|&(time, _)| time)
    }

    /// The keyframe at source time `current` is filled.
    pub(super) fn paint(&self, painter: &egui::Painter, current: Ticks) {
        let outline = Stroke::new(1.0, Color32::BLACK);
        painter.add(Shape::line(
            self.dots.clone(),
            Stroke::new(1.0, PATH.gamma_multiply(0.6)),
        ));
        for &dot in &self.dots {
            painter.circle_filled(dot, 1.5, PATH);
        }
        for &(time, at) in &self.keys {
            let square = Rect::from_center_size(at, Vec2::splat(KEY_SIZE));
            let fill = if time == current {
                PATH
            } else {
                Color32::from_black_alpha(160)
            };
            painter.rect_filled(square, 0.0, fill);
            painter.rect_stroke(
                square,
                0.0,
                Stroke::new(1.0, PATH),
                egui::StrokeKind::Inside,
            );
            painter.rect_stroke(square, 0.0, outline, egui::StrokeKind::Outside);
        }
    }
}
