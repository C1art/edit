/// Premiere's Generate effects drawn over a layer, all off by default. Points are
/// fractions of the picture's width and height, sizes are in its pixels, and colors
/// are gamma-encoded, 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Generate {
    pub checkerboard: Option<Checkerboard>,
    pub grid: Option<Grid>,
    pub circle: Option<Circle>,
    pub four_color: Option<FourColorGradient>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Checkerboard {
    pub anchor: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 3],
    /// 0 to 1.
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    pub anchor: [f32; 2],
    pub size: [f32; 2],
    pub border: f32,
    pub color: [f32; 3],
    /// 0 to 1.
    pub opacity: f32,
    pub invert: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    pub center: [f32; 2],
    pub radius: f32,
    pub feather: f32,
    pub color: [f32; 3],
    /// 0 to 1.
    pub opacity: f32,
    pub invert: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FourColorGradient {
    pub points: [[f32; 2]; 4],
    pub colors: [[f32; 3]; 4],
    /// How smoothly the colors run together: 1 is the classic falloff, higher smoother.
    pub blend: f32,
    /// 0 to 1.
    pub opacity: f32,
}

impl Generate {
    pub const NONE: Self = Self {
        checkerboard: None,
        grid: None,
        circle: None,
        four_color: None,
    };

    /// The shader's view, as thirteen vec4s; an effect is off at zero opacity.
    pub(crate) fn uniform(&self) -> [[f32; 4]; 13] {
        let flag = |on: bool| if on { 1.0 } else { 0.0 };
        let checker = self.checkerboard.unwrap_or(Checkerboard {
            anchor: [0.0; 2],
            size: [1.0; 2],
            color: [0.0; 3],
            opacity: 0.0,
        });
        let grid = self.grid.unwrap_or(Grid {
            anchor: [0.0; 2],
            size: [1.0; 2],
            border: 0.0,
            color: [0.0; 3],
            opacity: 0.0,
            invert: false,
        });
        let circle = self.circle.unwrap_or(Circle {
            center: [0.0; 2],
            radius: 0.0,
            feather: 0.0,
            color: [0.0; 3],
            opacity: 0.0,
            invert: false,
        });
        let four = self.four_color.unwrap_or(FourColorGradient {
            points: [[0.0; 2]; 4],
            colors: [[0.0; 3]; 4],
            blend: 1.0,
            opacity: 0.0,
        });
        let colored = |[r, g, b]: [f32; 3], w: f32| [r, g, b, w];
        let sides = |[width, height]: [f32; 2]| [width.max(1.0), height.max(1.0)];
        let [[c0x, c0y], [c1x, c1y], [c2x, c2y], [c3x, c3y]] = four.points;
        let [[ax, ay], [sx, sy]] = [checker.anchor, sides(checker.size)];
        let [[gx, gy], [gw, gh]] = [grid.anchor, sides(grid.size)];
        let [cx, cy] = circle.center;
        [
            [ax, ay, sx, sy],
            colored(checker.color, checker.opacity),
            [gx, gy, gw, gh],
            colored(grid.color, grid.opacity),
            [cx, cy, circle.radius.max(0.0), circle.feather.max(0.0)],
            colored(circle.color, circle.opacity),
            [
                grid.border.max(0.0),
                flag(grid.invert),
                flag(circle.invert),
                four.blend.max(0.01),
            ],
            [c0x, c0y, c1x, c1y],
            [c2x, c2y, c3x, c3y],
            colored(four.colors[0], four.opacity),
            colored(four.colors[1], 0.0),
            colored(four.colors[2], 0.0),
            colored(four.colors[3], 0.0),
        ]
    }
}
