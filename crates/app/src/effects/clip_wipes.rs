use timeline::{RadialWipeDirection, VideoEffect};

use super::look::Rows;

const COMPLETION: std::ops::RangeInclusive<f32> = 0.0..=100.0;
const ANGLE: std::ops::RangeInclusive<f32> = -360.0..=360.0;
const FEATHER: std::ops::RangeInclusive<f32> = 0.0..=500.0;

impl Rows {
    /// The controls of the Transition effects; other effects have none here.
    pub(super) fn clip_wipes(&mut self, ui: &mut egui::Ui, effect: &mut VideoEffect) {
        match effect {
            VideoEffect::LinearWipe {
                completion,
                angle,
                feather,
            } => {
                self.param(ui, "Transition Completion", completion, COMPLETION, "%");
                self.param(ui, "Wipe Angle", angle, ANGLE, "°");
                self.param(ui, "Feather", feather, FEATHER, "px");
            }
            VideoEffect::RadialWipe {
                completion,
                start_angle,
                center: [x, y],
                direction,
                feather,
            } => {
                self.param(ui, "Transition Completion", completion, COMPLETION, "%");
                self.param(ui, "Start Angle", start_angle, ANGLE, "°");
                self.param(ui, "Wipe Center X", x, -100.0..=200.0, "%");
                self.param(ui, "Wipe Center Y", y, -100.0..=200.0, "%");
                self.options(
                    ui,
                    "Wipe",
                    "radial_wipe_direction",
                    direction,
                    RadialWipeDirection::ALL,
                    RadialWipeDirection::name,
                );
                self.param(ui, "Feather", feather, FEATHER, "px");
            }
            VideoEffect::VenetianBlinds {
                completion,
                direction,
                width,
                feather,
            } => {
                self.param(ui, "Transition Completion", completion, COMPLETION, "%");
                self.param(ui, "Direction", direction, ANGLE, "°");
                self.param(ui, "Width", width, 1.0..=1000.0, "px");
                self.param(ui, "Feather", feather, FEATHER, "px");
            }
            VideoEffect::BlockDissolve {
                completion,
                block_width,
                block_height,
                feather,
            } => {
                self.param(ui, "Transition Completion", completion, COMPLETION, "%");
                self.param(ui, "Block Width", block_width, 1.0..=1000.0, "px");
                self.param(ui, "Block Height", block_height, 1.0..=1000.0, "px");
                self.param(ui, "Feather", feather, FEATHER, "px");
            }
            _ => {}
        }
    }
}
