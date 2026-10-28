/// Effects that reshape, smear or restyle a layer's picture, all off by default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PictureStyle {
    /// -1 to 1, on encoded values as a display shows them.
    pub brightness: f32,
    /// -1 (flat grey) to 1.
    pub contrast: f32,
    /// An exponent on encoded values.
    pub gamma: f32,
    /// Levels per channel; below 2 the picture stays smooth.
    pub posterize: f32,
    /// 0 to 1.
    pub noise: f32,
    /// Changes the grain from frame to frame.
    pub noise_seed: f32,
    /// 0 to 1: how much the outlines replace the picture.
    pub edges: f32,
    /// Light outlines on black instead of dark ones on white.
    pub edges_inverted: bool,
    /// Blocks across and down; below 1 the picture stays whole.
    pub mosaic: [f32; 2],
    /// Fractions of the picture it shifts right and down by, wrapping around.
    pub offset: [f32; 2],
    /// 0 to 1: how much the shifted picture replaces the original.
    pub offset_mix: f32,
    /// Radians the center turns by, the turn fading to none at the radius.
    pub twirl: f32,
    /// Fraction of half the picture's diagonal.
    pub twirl_radius: f32,
    /// -1 (pinched) to 1 (bulging).
    pub lens: f32,
    /// Canvas pixels of the fitted picture the blur reaches each way.
    pub blur_length: f32,
    /// Radians clockwise from vertical.
    pub blur_direction: f32,
    /// Pixels of the fitted picture the waves push it across their direction.
    pub wave_height: f32,
    /// Pixels of the fitted picture from one wave to the next.
    pub wave_width: f32,
    /// Radians clockwise from up that the waves run along.
    pub wave_direction: f32,
    /// Waves travelled so far, so they move as the clip plays.
    pub wave_phase: f32,
    /// Pixels of the fitted picture right and down the shadow falls.
    pub shadow_offset: [f32; 2],
    /// Pixels of the fitted picture the shadow's edge softens over.
    pub shadow_softness: f32,
    /// 0 (no shadow) to 1.
    pub shadow_opacity: f32,
    /// Linear.
    pub shadow_color: [f32; 3],
}

impl PictureStyle {
    pub const NONE: Self = Self {
        brightness: 0.0,
        contrast: 0.0,
        gamma: 1.0,
        posterize: 0.0,
        noise: 0.0,
        noise_seed: 0.0,
        edges: 0.0,
        edges_inverted: false,
        mosaic: [0.0; 2],
        offset: [0.0; 2],
        offset_mix: 0.0,
        twirl: 0.0,
        twirl_radius: 0.0,
        lens: 0.0,
        blur_length: 0.0,
        blur_direction: 0.0,
        wave_height: 0.0,
        wave_width: 1.0,
        wave_direction: 0.0,
        wave_phase: 0.0,
        shadow_offset: [0.0; 2],
        shadow_softness: 0.0,
        shadow_opacity: 0.0,
        shadow_color: [0.0; 3],
    };

    /// The shader's view, as eight vec4s.
    pub(crate) fn uniform(&self) -> [f32; 32] {
        let inverted = if self.edges_inverted { 1.0 } else { 0.0 };
        let [mosaic_x, mosaic_y] = self.mosaic;
        let [offset_x, offset_y] = self.offset;
        let [shadow_x, shadow_y] = self.shadow_offset;
        let [red, green, blue] = self.shadow_color;
        [
            self.brightness,
            self.contrast,
            self.gamma.max(0.01),
            self.posterize,
            self.noise,
            self.noise_seed,
            self.edges,
            inverted,
            offset_x,
            offset_y,
            self.offset_mix,
            self.lens.clamp(-1.0, 1.0),
            mosaic_x,
            mosaic_y,
            self.twirl,
            self.twirl_radius.max(0.0),
            self.blur_length.max(0.0),
            self.blur_direction,
            0.0,
            0.0,
            self.wave_height,
            self.wave_width.max(1.0),
            self.wave_direction,
            self.wave_phase,
            shadow_x,
            shadow_y,
            self.shadow_softness.max(0.0),
            self.shadow_opacity.clamp(0.0, 1.0),
            red,
            green,
            blue,
            0.0,
        ]
    }
}
