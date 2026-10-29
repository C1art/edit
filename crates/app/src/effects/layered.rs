use timeline::{DifferenceView, VideoEffect};

use super::look::Rows;

impl Rows {
    /// The controls of Difference Matte, Compound Blur and Texturize; other effects have
    /// none here.
    pub(super) fn layered(
        &mut self,
        ui: &mut egui::Ui,
        effect: &mut VideoEffect,
        video_tracks: usize,
    ) {
        match effect {
            VideoEffect::DifferenceMatte(matte) => {
                self.options(
                    ui,
                    "View",
                    "difference_view",
                    &mut matte.view,
                    DifferenceView::ALL,
                    DifferenceView::name,
                );
                let layer = &mut matte.difference_layer;
                self.track_choice(ui, "Difference Layer", layer, video_tracks);
                let tolerance = &mut matte.matching_tolerance;
                self.param(ui, "Matching Tolerance", tolerance, 0.0..=100.0, "%");
                let softness = &mut matte.matching_softness;
                self.param(ui, "Matching Softness", softness, 0.0..=100.0, "%");
                let blur = &mut matte.blur_before_difference;
                self.param(ui, "Blur Before Difference", blur, 0.0..=100.0, "px");
            }
            VideoEffect::CompoundBlur(blur) => {
                let layer = &mut blur.blur_layer;
                self.track_choice(ui, "Blur Layer", layer, video_tracks);
                self.param(
                    ui,
                    "Maximum Blur",
                    &mut blur.maximum_blur,
                    0.0..=100.0,
                    "px",
                );
                let invert = &mut blur.invert_blur;
                self.choice(ui, "Invert Blur", |ui| ui.checkbox(invert, "").changed());
            }
            VideoEffect::Texturize(texture) => {
                let layer = &mut texture.texture_layer;
                self.track_choice(ui, "Texture Layer", layer, video_tracks);
                let direction = &mut texture.light_direction;
                self.param(ui, "Light Direction", direction, -360.0..=360.0, "°");
                let contrast = &mut texture.texture_contrast;
                self.param(ui, "Texture Contrast", contrast, 0.0..=2.0, "");
            }
            _ => {}
        }
    }

    /// A row choosing one of the sequence's video tracks, or none.
    pub(super) fn track_choice(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        track: &mut Option<usize>,
        video_tracks: usize,
    ) {
        self.choice(ui, label, |ui| {
            let name = |track: Option<usize>| {
                track.map_or("None".to_owned(), |index| format!("Video {}", index + 1))
            };
            let mut chosen = false;
            egui::ComboBox::from_id_salt(label)
                .selected_text(name(*track))
                .show_ui(ui, |ui| {
                    let tracks = std::iter::once(None).chain((0..video_tracks).map(Some));
                    for option in tracks {
                        chosen |= ui.selectable_value(track, option, name(option)).clicked();
                    }
                });
            chosen
        });
    }
}
