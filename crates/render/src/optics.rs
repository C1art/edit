/// Lens Flare and Magnify, both off by default. Points are fractions of the picture's
/// width and height, sizes are in its pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Optics {
    pub lens_flare: Option<LensFlare>,
    pub magnify: Option<Magnify>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LensFlare {
    pub center: [f32; 2],
    /// 1 is Premiere's 100%.
    pub brightness: f32,
    pub lens: FlareLens,
    /// 0 to 1: how much of the flare is left out.
    pub blend: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlareLens {
    Zoom,
    Prime35,
    Prime105,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Magnify {
    pub center: [f32; 2],
    /// 2 doubles the picture's size.
    pub magnification: f32,
    /// The circle's radius, or half the square's side.
    pub size: f32,
    pub feather: f32,
    /// 0 to 1.
    pub opacity: f32,
    pub square: bool,
}

impl Optics {
    pub const NONE: Self = Self {
        lens_flare: None,
        magnify: None,
    };

    /// The shader's view, as three vec4s; each effect is off at zero brightness or
    /// opacity.
    pub(crate) fn uniform(&self) -> [[f32; 4]; 3] {
        let flare = self.lens_flare.unwrap_or(LensFlare {
            center: [0.0; 2],
            brightness: 0.0,
            lens: FlareLens::Zoom,
            blend: 1.0,
        });
        let magnify = self.magnify.unwrap_or(Magnify {
            center: [0.0; 2],
            magnification: 1.0,
            size: 0.0,
            feather: 0.0,
            opacity: 0.0,
            square: false,
        });
        let lens = match flare.lens {
            FlareLens::Zoom => 0.0,
            FlareLens::Prime35 => 1.0,
            FlareLens::Prime105 => 2.0,
        };
        let [fx, fy] = flare.center;
        let [mx, my] = magnify.center;
        [
            [fx, fy, flare.brightness.max(0.0), lens],
            [
                mx,
                my,
                magnify.magnification.max(0.01),
                magnify.size.max(0.0),
            ],
            [
                magnify.feather.max(0.0),
                if magnify.square { 1.0 } else { 0.0 },
                magnify.opacity.clamp(0.0, 1.0),
                flare.blend.clamp(0.0, 1.0),
            ],
        ]
    }
}
