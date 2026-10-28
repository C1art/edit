use timeline::{Side, Sweep, WipePattern};

/// Part of a layer a transition shows, in canvas space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerWipe {
    pub pattern: WipePattern,
    pub from: Side,
    /// 0 hides the layer and 1 shows all of it.
    pub progress: f32,
    pub sweep: Sweep,
    /// A share of the canvas height.
    pub border: f32,
    /// Linear.
    pub border_color: [f32; 3],
}

impl LayerWipe {
    fn uniform(&self) -> [f32; 8] {
        // The shader numbers patterns in declaration order, and sides left, top, right,
        // bottom.
        let side = match self.from {
            Side::Left => 0.0,
            Side::Top => 1.0,
            Side::Right => 2.0,
            Side::Bottom => 3.0,
        };
        let reversed = if self.sweep == Sweep::Reverse {
            1.0
        } else {
            0.0
        };
        let [r, g, b] = self.border_color;
        [
            self.pattern as u32 as f32,
            side,
            self.progress.clamp(0.0, 1.0),
            reversed,
            self.border.max(0.0),
            r,
            g,
            b,
        ]
    }
}

pub(crate) fn uniform(wipes: &[Option<LayerWipe>; 2]) -> impl Iterator<Item = f32> + '_ {
    const UNUSED: [f32; 8] = [-1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    wipes
        .iter()
        .flat_map(|wipe| wipe.as_ref().map_or(UNUSED, LayerWipe::uniform))
}
