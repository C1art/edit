/// The Transition category's clip effects, each hiding part of the picture and off when
/// absent. Completions run from 0 to 1, angles are radians clockwise from up, points
/// are fractions of the picture's width and height and lengths are in its pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipWipes {
    pub linear: Option<LinearWipe>,
    pub radial: Option<RadialWipe>,
    pub blinds: Option<VenetianBlinds>,
    pub blocks: Option<BlockDissolve>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearWipe {
    pub completion: f32,
    /// The way the wipe travels.
    pub angle: f32,
    pub feather: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadialWipe {
    pub completion: f32,
    pub start_angle: f32,
    pub center: [f32; 2],
    pub sweep: RadialSweep,
    pub feather: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadialSweep {
    Clockwise,
    Counterclockwise,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VenetianBlinds {
    pub completion: f32,
    /// The way each stripe closes.
    pub direction: f32,
    pub width: f32,
    pub feather: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockDissolve {
    pub completion: f32,
    pub size: [f32; 2],
    pub feather: f32,
}

impl ClipWipes {
    pub const NONE: Self = Self {
        linear: None,
        radial: None,
        blinds: None,
        blocks: None,
    };

    /// The shader's view, as five vec4s; each effect is off at zero completion.
    pub(crate) fn uniform(&self) -> [[f32; 4]; 5] {
        let linear = self.linear.map_or([0.0; 4], |wipe| {
            [
                completion(wipe.completion),
                wipe.angle,
                feather(wipe.feather),
                0.0,
            ]
        });
        let (radial, radial_detail) = self.radial.map_or(([0.0; 4], [0.0; 4]), |wipe| {
            let [x, y] = wipe.center;
            let sweep = match wipe.sweep {
                RadialSweep::Clockwise => 0.0,
                RadialSweep::Counterclockwise => 1.0,
                RadialSweep::Both => 2.0,
            };
            (
                [x, y, completion(wipe.completion), wipe.start_angle],
                [feather(wipe.feather), sweep, 0.0, 0.0],
            )
        });
        let blinds = self.blinds.map_or([0.0; 4], |blinds| {
            [
                completion(blinds.completion),
                blinds.direction,
                blinds.width.max(1.0),
                feather(blinds.feather),
            ]
        });
        let blocks = self.blocks.map_or([0.0; 4], |blocks| {
            let [width, height] = blocks.size;
            [
                completion(blocks.completion),
                width.max(1.0),
                height.max(1.0),
                feather(blocks.feather),
            ]
        });
        [linear, radial, radial_detail, blinds, blocks]
    }
}

fn completion(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn feather(value: f32) -> f32 {
    value.max(0.0)
}
