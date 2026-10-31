/// Radial Shadow's projection; its color, opacity and softness are the style's shadow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointShadow {
    /// Fractions of the picture's width and height.
    pub light: [f32; 2],
    /// How much larger than the picture its shadow falls; 1 lies right under it.
    pub scale: f32,
    pub shadow_only: bool,
    /// For a glass edge shadow, 0 to 1: how much of the picture's color it takes on.
    pub glass_influence: Option<f32>,
}

impl PointShadow {
    pub const NONE: Option<Self> = None;

    /// The shader's view, as two vec4s; off at a zero flag.
    pub(crate) fn uniform(shadow: Option<&Self>) -> [[f32; 4]; 2] {
        let Some(shadow) = shadow else {
            return [[0.0; 4]; 2];
        };
        let [x, y] = shadow.light;
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        [
            [x, y, shadow.scale.max(1e-3), 1.0],
            [
                flag(shadow.shadow_only),
                flag(shadow.glass_influence.is_some()),
                shadow.glass_influence.unwrap_or(0.0).clamp(0.0, 1.0),
                0.0,
            ],
        ]
    }
}
