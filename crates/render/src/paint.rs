/// Brush Strokes, Eyedropper Fill and SDR Conform, all off by default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paint {
    pub brush: Option<Brush>,
    pub eyedropper: Option<Eyedropper>,
    pub conform: Option<Conform>,
}

/// Sizes in the picture's pixels, the angle in radians clockwise from up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Brush {
    pub angle: f32,
    pub size: f32,
    pub length: f32,
    /// 0 to 1: up to how far each stroke turns from the angle, at most a quarter turn
    /// either way.
    pub randomness: f32,
    pub surface: BrushSurface,
    /// 0 to 1.
    pub blend: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushSurface {
    Original,
    Transparent,
    White,
    Black,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Eyedropper {
    /// A fraction of the picture.
    pub point: [f32; 2],
    /// Pixels around the point averaged together.
    pub radius: f32,
    pub keep_alpha: bool,
    /// 0 to 1.
    pub blend: f32,
}

/// Exposure and contrast, each −1 to 1, then a soft knee easing the top `knee`
/// fraction of the range into white.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conform {
    pub brightness: f32,
    pub contrast: f32,
    pub knee: f32,
}

impl Paint {
    pub const NONE: Self = Self {
        brush: None,
        eyedropper: None,
        conform: None,
    };

    /// The shader's view, as five vec4s; each effect is off at a zero flag.
    pub(crate) fn uniform(&self) -> [[f32; 4]; 5] {
        let mut uniform = [[0.0; 4]; 5];
        if let Some(brush) = self.brush {
            let surface = match brush.surface {
                BrushSurface::Original => 1.0,
                BrushSurface::Transparent => 2.0,
                BrushSurface::White => 3.0,
                BrushSurface::Black => 4.0,
            };
            uniform[0] = [
                brush.angle,
                brush.size.max(1.0),
                brush.length.max(1.0),
                brush.randomness.clamp(0.0, 1.0),
            ];
            uniform[1] = [surface, brush.blend.clamp(0.0, 1.0), 0.0, 0.0];
        }
        if let Some(fill) = self.eyedropper {
            uniform[2] = [fill.point[0], fill.point[1], fill.radius.max(0.0), 1.0];
            uniform[3] = [
                fill.blend.clamp(0.0, 1.0),
                f32::from(u8::from(fill.keep_alpha)),
                0.0,
                0.0,
            ];
        }
        if let Some(conform) = self.conform {
            uniform[4] = [
                conform.brightness.clamp(-1.0, 1.0),
                conform.contrast.clamp(-1.0, 1.0),
                conform.knee.clamp(0.0, 0.95),
                1.0,
            ];
        }
        uniform
    }
}
