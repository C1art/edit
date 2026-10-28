/// Effects that key, decolor or replace a layer's colors, all off by default. Colors
/// here are gamma-encoded, 0 to 1, as these effects compare and blend them so.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recolor {
    pub color_key: Option<ColorKey>,
    pub leave_color: Option<LeaveColor>,
    pub ramp: Option<Ramp>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorKey {
    pub color: [f32; 3],
    /// 0 to 1 of the largest distance between colors.
    pub tolerance: f32,
    pub softness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeaveColor {
    pub color: [f32; 3],
    /// 0 to 1: how grey the other colors turn.
    pub amount: f32,
    /// 0 to 1 of the largest distance between colors, or of half a turn of hue.
    pub tolerance: f32,
    pub softness: f32,
    pub by_hue: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    /// Fractions of the picture's width and height.
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub start_color: [f32; 3],
    pub end_color: [f32; 3],
    pub radial: bool,
    /// 0 to 1: how much of the original shows through.
    pub blend: f32,
}

impl Recolor {
    pub const NONE: Self = Self {
        color_key: None,
        leave_color: None,
        ramp: None,
    };

    /// The shader's view, as six vec4s; a negative tolerance turns a key off, and a
    /// ramp is off where none of it shows.
    pub(crate) fn uniform(&self) -> [f32; 24] {
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let [kr, kg, kb, key_tolerance, key_softness] =
            self.color_key.map_or([0.0, 0.0, 0.0, -1.0, 0.0], |key| {
                let [r, g, b] = key.color;
                [r, g, b, key.tolerance, key.softness]
            });
        let leave = self.leave_color.unwrap_or(LeaveColor {
            color: [0.0; 3],
            amount: 0.0,
            tolerance: -1.0,
            softness: 0.0,
            by_hue: false,
        });
        let [lr, lg, lb] = leave.color;
        let ramp = self.ramp.unwrap_or(Ramp {
            start: [0.0; 2],
            end: [0.0, 1.0],
            start_color: [0.0; 3],
            end_color: [0.0; 3],
            radial: false,
            blend: 1.0,
        });
        let [sr, sg, sb] = ramp.start_color;
        let [er, eg, eb] = ramp.end_color;
        [
            kr,
            kg,
            kb,
            key_tolerance,
            lr,
            lg,
            lb,
            leave.tolerance,
            leave.amount,
            leave.softness,
            flag(leave.by_hue),
            key_softness,
            ramp.start[0],
            ramp.start[1],
            ramp.end[0],
            ramp.end[1],
            sr,
            sg,
            sb,
            flag(ramp.radial),
            er,
            eg,
            eb,
            1.0 - ramp.blend.clamp(0.0, 1.0),
        ]
    }
}
