use timeline::{ChannelBlur, Echo, EchoOperator, VideoEffect};

use super::look::Rows;

const BLURRINESS: std::ops::RangeInclusive<f32> = 0.0..=500.0;
const ECHO_SPAN: std::ops::RangeInclusive<f32> = -30.0..=30.0;

impl Rows {
    /// The controls of Channel Blur, Camera Blur and Echo; other effects have none here.
    pub(super) fn blur_echo(&mut self, ui: &mut egui::Ui, effect: &mut VideoEffect) {
        match effect {
            VideoEffect::ChannelBlur(ChannelBlur {
                red,
                green,
                blue,
                alpha,
            }) => {
                self.param(ui, "Red Blurriness", red, BLURRINESS, "px");
                self.param(ui, "Green Blurriness", green, BLURRINESS, "px");
                self.param(ui, "Blue Blurriness", blue, BLURRINESS, "px");
                self.param(ui, "Alpha Blurriness", alpha, BLURRINESS, "px");
            }
            VideoEffect::CameraBlur { percent } => {
                self.param(ui, "Percent Blur", percent, 0.0..=100.0, "%");
            }
            VideoEffect::Echo(Echo {
                echo_time,
                echoes,
                starting_intensity,
                decay,
                operator,
            }) => {
                self.param(ui, "Echo Time", echo_time, ECHO_SPAN, "frames");
                self.param(ui, "Number of Echoes", echoes, 0.0..=30.0, "");
                self.param(ui, "Starting Intensity", starting_intensity, 0.0..=1.0, "");
                self.param(ui, "Decay", decay, 0.0..=1.0, "");
                self.options(
                    ui,
                    "Echo Operator",
                    "echo_operator",
                    operator,
                    EchoOperator::ALL,
                    EchoOperator::name,
                );
            }
            _ => {}
        }
    }
}
