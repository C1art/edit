/// Effects that key, decolor or replace a layer's colors, all off by default. Colors
/// here are gamma-encoded, 0 to 1, as these effects compare and blend them so.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recolor {
    pub color_key: Option<ColorKey>,
    pub leave_color: Option<LeaveColor>,
    pub ramp: Option<Ramp>,
    pub channel_mixer: Option<ChannelMixer>,
    pub change_to_color: Option<ChangeToColor>,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelMixer {
    /// Red, green and blue each made from shares of the three, then a constant.
    pub rows: [[f32; 4]; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChangeToColor {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub change: ColorChange,
    /// 0 to 1 of half a turn of hue, of lightness and of saturation.
    pub tolerance: [f32; 3],
    /// Fraction of each tolerance past it that the match fades over.
    pub softness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChange {
    Hue,
    HueLightness,
    HueLightnessSaturation,
    Colorize,
}

impl Recolor {
    pub const NONE: Self = Self {
        color_key: None,
        leave_color: None,
        ramp: None,
        channel_mixer: None,
        change_to_color: None,
    };

    /// The shader's view, as twelve vec4s.
    pub(crate) fn uniform(&self) -> [f32; 48] {
        let mut uniform = [0.0; 48];
        let values = self.keys_and_ramp().into_iter().chain(self.remap());
        for (slot, value) in uniform.iter_mut().zip(values) {
            *slot = value;
        }
        uniform
    }

    /// The channel mixer's rows, then Change to Color's colors and tolerances, the
    /// mixer's on flag last; a negative hue tolerance turns Change to Color off.
    fn remap(&self) -> [f32; 24] {
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ];
        let [red, green, blue] = self.channel_mixer.map_or(identity, |mixer| mixer.rows);
        let change = self.change_to_color.unwrap_or(ChangeToColor {
            from: [0.0; 3],
            to: [0.0; 3],
            change: ColorChange::Hue,
            tolerance: [-1.0, 0.0, 0.0],
            softness: 0.0,
        });
        let [fr, fg, fb] = change.from;
        let [tr, tg, tb] = change.to;
        let [hue, lightness, saturation] = change.tolerance;
        let mode = match change.change {
            ColorChange::Hue => 0.0,
            ColorChange::HueLightness => 1.0,
            ColorChange::HueLightnessSaturation => 2.0,
            ColorChange::Colorize => 3.0,
        };
        let [r0, r1, r2, r3] = red;
        let [g0, g1, g2, g3] = green;
        let [b0, b1, b2, b3] = blue;
        [
            r0,
            r1,
            r2,
            r3,
            g0,
            g1,
            g2,
            g3,
            b0,
            b1,
            b2,
            b3,
            fr,
            fg,
            fb,
            hue,
            tr,
            tg,
            tb,
            mode,
            lightness,
            saturation,
            change.softness.max(0.0),
            flag(self.channel_mixer.is_some()),
        ]
    }

    /// A negative tolerance turns a key off, and a ramp is off where none of it shows.
    fn keys_and_ramp(&self) -> [f32; 24] {
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
