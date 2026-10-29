/// Roughen Edges, Bevel Edges, Bevel Alpha, Alpha Glow and Color Emboss, all off by
/// default. Sizes are in the picture's pixels, angles in radians clockwise from up and
/// colors linear RGB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edges {
    pub roughen: Option<RoughenEdges>,
    pub bevel: Option<Bevel>,
    pub bevel_alpha: Option<Bevel>,
    pub alpha_glow: Option<AlphaGlow>,
    pub color_emboss: Option<ColorEmboss>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoughenEdges {
    /// How far in the edge is worn.
    pub border: f32,
    /// 0 to 10: how crisp the worn edge is.
    pub sharpness: f32,
    /// How much the fractal shapes the edge, 0 for an even inset to 1.
    pub fractal_influence: f32,
    /// Pixels across the fractal's largest features.
    pub scale: f32,
    pub offset: [f32; 2],
    /// Layers of finer fractal, 1 to 10.
    pub complexity: f32,
    /// Turning it changes the fractal smoothly.
    pub evolution: f32,
    /// Shown along the worn edge, for Roughen Color.
    pub edge_color: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bevel {
    /// Bevel Edges' share of the picture's shorter side, or Bevel Alpha's pixels.
    pub thickness: f32,
    pub light_angle: f32,
    pub light_color: [f32; 3],
    /// 0 to 1.
    pub light_intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlphaGlow {
    pub size: f32,
    /// 0 to 1.
    pub brightness: f32,
    pub start_color: [f32; 3],
    /// The color the glow turns toward at its outer edge.
    pub end_color: Option<[f32; 3]>,
    /// Whether the glow fades away toward its outer edge.
    pub fade_out: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorEmboss {
    pub direction: f32,
    pub relief: f32,
    /// 1 for the plain relief.
    pub contrast: f32,
    /// 0 to 1: how much of the original shows through.
    pub blend: f32,
}

impl Edges {
    pub const NONE: Self = Self {
        roughen: None,
        bevel: None,
        bevel_alpha: None,
        alpha_glow: None,
        color_emboss: None,
    };

    /// The shader's view, as eleven vec4s; each effect is off at zero size or amount.
    pub(crate) fn uniform(&self) -> [[f32; 4]; 11] {
        let [roughen, roughen_noise, roughen_color] = self.roughen.map_or([[0.0; 4]; 3], |rough| {
            // Sharper edges blend over a narrower span of the depth into the picture.
            let softness = 0.25 / (1.0 + 2.0 * rough.sharpness.max(0.0));
            let [x, y] = rough.offset;
            let [r, g, b] = rough.edge_color.unwrap_or([0.0; 3]);
            [
                [
                    rough.border.max(0.0),
                    softness,
                    rough.scale.max(1.0),
                    rough.complexity.clamp(1.0, 10.0),
                ],
                [
                    rough.evolution,
                    rough.fractal_influence.clamp(0.0, 1.0),
                    x,
                    y,
                ],
                [r, g, b, f32::from(u8::from(rough.edge_color.is_some()))],
            ]
        });
        let bevel = |bevel: Option<Bevel>| {
            bevel.map_or([[0.0; 4]; 2], |bevel| {
                let [r, g, b] = bevel.light_color;
                [
                    [
                        bevel.thickness.max(0.0),
                        bevel.light_angle,
                        bevel.light_intensity.clamp(0.0, 1.0),
                        0.0,
                    ],
                    [r, g, b, 0.0],
                ]
            })
        };
        let [bevel_edges, bevel_edges_color] = bevel(self.bevel);
        let [bevel_alpha, bevel_alpha_color] = bevel(self.bevel_alpha);
        let [glow, glow_start, glow_end] = self.alpha_glow.map_or([[0.0; 4]; 3], |glow| {
            let [r, g, b] = glow.start_color;
            let [end_r, end_g, end_b] = glow.end_color.unwrap_or(glow.start_color);
            [
                [
                    glow.size.max(0.0),
                    glow.brightness.clamp(0.0, 1.0),
                    0.0,
                    f32::from(u8::from(glow.fade_out)),
                ],
                [r, g, b, 0.0],
                [end_r, end_g, end_b, 0.0],
            ]
        });
        let emboss = self.color_emboss.map_or([0.0; 4], |emboss| {
            [
                emboss.direction,
                emboss.relief.max(0.0),
                emboss.contrast.max(0.0),
                1.0 - emboss.blend.clamp(0.0, 1.0),
            ]
        });
        [
            roughen,
            roughen_noise,
            roughen_color,
            bevel_edges,
            bevel_edges_color,
            bevel_alpha,
            bevel_alpha_color,
            glow,
            glow_start,
            glow_end,
            emboss,
        ]
    }
}
