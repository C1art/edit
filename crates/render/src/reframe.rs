use crate::placement::{fit_scale, LayerLook};

/// Auto Reframe: the picture enlarged until it fills the canvas, then moved to keep its
/// subject in the middle as far as its edges allow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reframe {
    /// Where the subject is, as fractions of the picture from its top left.
    pub subject: [f32; 2],
    /// Relative to just filling the canvas.
    pub zoom: f32,
}

impl LayerLook {
    /// This look with its reframing folded into its scale and offset, for a picture
    /// shown `shown` pixels big on a canvas of `canvas`.
    pub(crate) fn reframed(&self, shown: (u32, u32), canvas: (u32, u32)) -> Self {
        let Some(reframe) = self.reframe else {
            return *self;
        };
        let fit = fit_scale(shown, canvas);
        let fitted = [shown.0, shown.1].map(|side| side as f32 * fit);
        let canvas = [canvas.0, canvas.1].map(|side| side as f32);
        let fill = (0..2)
            .map(|axis| canvas[axis] / fitted[axis].max(1.0))
            .fold(1.0, f32::max)
            * reframe.zoom.max(0.01);
        let scale = self.scale.map(|scale| scale * fill);
        let offset = [0, 1].map(|axis| {
            let size = (fitted[axis] * scale[axis]).abs();
            let slack = ((size - canvas[axis]) / 2.0).max(0.0);
            let pan = (0.5 - reframe.subject[axis]) * size;
            self.offset[axis] + pan.clamp(-slack, slack)
        });
        Self {
            scale,
            offset,
            reframe: None,
            ..*self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_picture_fills_a_tall_canvas_and_pans_to_its_subject_within_its_edges() {
        let reframed = |subject: [f32; 2]| {
            let look = LayerLook {
                reframe: Some(Reframe { subject, zoom: 1.0 }),
                ..LayerLook::IDENTITY
            };
            look.reframed((1920, 1080), (1080, 1920))
        };
        // Fitted 1080 by 607.5, filled 3413.3 by 1920: 1166.7 canvas pixels of slack
        // each way across.
        let centered = reframed([0.5, 0.5]);
        assert!((centered.scale[0] - 1920.0 / 607.5).abs() < 1e-3);
        assert_eq!(centered.offset, [0.0, 0.0]);
        let right = reframed([0.75, 0.5]);
        assert!((right.offset[0] + 3413.333 * 0.25).abs() < 0.1, "{right:?}");
        let edge = reframed([1.0, 0.0]);
        assert!((edge.offset[0] + 1166.667).abs() < 0.1, "{edge:?}");
        assert_eq!(edge.offset[1], 0.0);
    }
}
