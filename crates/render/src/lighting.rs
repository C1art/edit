/// Lighting Effects, off by default. Points and radii are fractions of the picture's
/// width and height, and colors are linear.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lighting {
    pub lights: [Option<Lamp>; LAMPS],
    pub ambience: [f32; 3],
    /// 0 to 1, as are the gloss and the material.
    pub ambience_intensity: f32,
    pub gloss: f32,
    /// 0 for plastic, highlights in the light's color, to 1 for metal, in the picture's.
    pub material: f32,
    /// A factor on the lit picture.
    pub exposure: f32,
    /// How steeply the picture's brightness raises the surface; 0 keeps it flat.
    pub bump: f32,
    pub white_is_high: bool,
}

pub const LAMPS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lamp {
    pub kind: LampKind,
    pub color: [f32; 3],
    pub center: [f32; 2],
    /// Of the picture's width.
    pub major_radius: f32,
    pub minor_radius: f32,
    /// Radians.
    pub angle: f32,
    /// 1 lights the picture as it is.
    pub intensity: f32,
    /// 0 to 1: where the spotlight starts fading, as a fraction of its radius.
    pub focus: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LampKind {
    Directional,
    Omni,
    Spotlight,
}

impl Lighting {
    pub const NONE: Option<Self> = None;

    /// The shader's view, as eighteen vec4s: three a light, off at a zero kind, then
    /// the surface; the whole effect is off at a zero flag.
    pub(crate) fn uniform(lighting: Option<&Self>) -> [[f32; 4]; 18] {
        let mut out = [[0.0; 4]; 18];
        let Some(lighting) = lighting else {
            return out;
        };
        for (slots, lamp) in out.as_chunks_mut::<3>().0.iter_mut().zip(&lighting.lights) {
            let Some(lamp) = lamp else { continue };
            let kind = match lamp.kind {
                LampKind::Directional => 1.0,
                LampKind::Omni => 2.0,
                LampKind::Spotlight => 3.0,
            };
            let [cx, cy] = lamp.center;
            let [r, g, b] = lamp.color;
            slots[0] = [kind, cx, cy, lamp.intensity];
            slots[1] = [r, g, b, lamp.focus.clamp(0.0, 1.0)];
            slots[2] = [
                lamp.major_radius.max(1e-3),
                lamp.minor_radius.max(1e-3),
                lamp.angle,
                0.0,
            ];
        }
        let [r, g, b] = lighting.ambience;
        out[15] = [r, g, b, lighting.ambience_intensity];
        out[16] = [
            lighting.gloss.clamp(0.0, 1.0),
            lighting.material.clamp(0.0, 1.0),
            lighting.exposure.max(0.0),
            lighting.bump.max(0.0),
        ];
        out[17] = [
            1.0,
            if lighting.white_is_high { 1.0 } else { 0.0 },
            0.0,
            0.0,
        ];
        out
    }
}
