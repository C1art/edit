/// A correction limited to the colors a hue, saturation and lightness key selects.
/// Hue is in turns from red; the ranges are fractions of gamma-encoded HSL.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondaryAdjust {
    pub hue: f32,
    /// Turns either side of the hue fully keyed.
    pub hue_width: f32,
    pub saturation_range: [f32; 2],
    pub lightness_range: [f32; 2],
    /// How far beyond each range the key fades out, as a fraction of it.
    pub softness: f32,
    pub inverted: bool,
    pub show_mask: bool,
    /// -1 (cool) to 1 (warm).
    pub temperature: f32,
    /// -1 (green) to 1 (magenta).
    pub tint: f32,
    /// -1 to 1.
    pub contrast: f32,
    /// 1 leaves color as is.
    pub saturation: f32,
    /// A push toward blue and red (Cb and Cr, -1 to 1) and a brightness offset.
    pub wheel: [f32; 3],
}

/// What the shader reads to tell the key's states apart.
enum KeyMode {
    Off = 0,
    On = 1,
    ShowMask = 2,
}

impl SecondaryAdjust {
    pub(crate) fn uniform(adjust: Option<&Self>) -> [f32; 16] {
        let Some(adjust) = adjust else {
            let mut off = [0.0; 16];
            off[3] = KeyMode::Off as u8 as f32;
            return off;
        };
        let mode = if adjust.show_mask {
            KeyMode::ShowMask
        } else {
            KeyMode::On
        };
        let [blue, red, level] = adjust.wheel;
        let [saturation_low, saturation_high] = adjust.saturation_range;
        let [lightness_low, lightness_high] = adjust.lightness_range;
        [
            adjust.hue,
            adjust.hue_width,
            adjust.softness,
            mode as u8 as f32,
            saturation_low,
            saturation_high,
            lightness_low,
            lightness_high,
            adjust.temperature,
            adjust.tint,
            adjust.contrast,
            adjust.saturation,
            blue,
            red,
            level,
            if adjust.inverted { 1.0 } else { 0.0 },
        ]
    }
}
