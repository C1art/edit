//! Built-in creative looks: settings of the grading panel's Creative controls that give
//! a picture a style without a LUT file.

use crate::{ColorCorrection, Param};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreativeLook {
    TealOrange,
    WarmGlow,
    CoolNight,
    Vintage,
    FadedMatte,
    Vivid,
    Monochrome,
    Sepia,
}

/// In hundredths, as [`Param`] keeps them; tints are `[blue, red]`.
struct Recipe {
    faded_film: i32,
    vibrance: i32,
    saturation: i32,
    shadow_tint: [i32; 2],
    highlight_tint: [i32; 2],
}

const NEUTRAL: Recipe = Recipe {
    faded_film: 0,
    vibrance: 0,
    saturation: 10_000,
    shadow_tint: [0, 0],
    highlight_tint: [0, 0],
};

impl CreativeLook {
    pub const ALL: [Self; 8] = [
        Self::TealOrange,
        Self::WarmGlow,
        Self::CoolNight,
        Self::Vintage,
        Self::FadedMatte,
        Self::Vivid,
        Self::Monochrome,
        Self::Sepia,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::TealOrange => "Teal & Orange",
            Self::WarmGlow => "Warm Glow",
            Self::CoolNight => "Cool Night",
            Self::Vintage => "Vintage",
            Self::FadedMatte => "Faded Matte",
            Self::Vivid => "Vivid",
            Self::Monochrome => "Monochrome",
            Self::Sepia => "Sepia",
        }
    }

    const fn recipe(self) -> Recipe {
        match self {
            Self::TealOrange => Recipe {
                vibrance: 1_500,
                shadow_tint: [3_000, -2_000],
                highlight_tint: [-3_000, 2_000],
                ..NEUTRAL
            },
            Self::WarmGlow => Recipe {
                faded_film: 1_000,
                vibrance: 1_000,
                shadow_tint: [-1_000, 1_000],
                highlight_tint: [-3_000, 1_500],
                ..NEUTRAL
            },
            Self::CoolNight => Recipe {
                saturation: 8_000,
                shadow_tint: [3_500, -1_000],
                highlight_tint: [1_500, -500],
                ..NEUTRAL
            },
            Self::Vintage => Recipe {
                faded_film: 4_000,
                vibrance: -1_000,
                saturation: 7_500,
                shadow_tint: [-1_000, 1_500],
                highlight_tint: [-2_500, 1_000],
            },
            Self::FadedMatte => Recipe {
                faded_film: 6_000,
                saturation: 8_500,
                ..NEUTRAL
            },
            Self::Vivid => Recipe {
                vibrance: 4_000,
                saturation: 11_500,
                ..NEUTRAL
            },
            Self::Monochrome => Recipe {
                saturation: 0,
                ..NEUTRAL
            },
            Self::Sepia => Recipe {
                saturation: 0,
                shadow_tint: [-2_500, 1_500],
                highlight_tint: [-3_500, 1_500],
                ..NEUTRAL
            },
        }
    }

    /// Sets the Creative controls to `look`, or back to neutral for `None`, leaving the
    /// tint balance and any look LUT alone.
    pub fn apply(look: Option<Self>, color: &mut ColorCorrection) {
        let recipe = look.map_or(NEUTRAL, Self::recipe);
        for (param, value) in creative_params(color).into_iter().zip(recipe.values()) {
            *param = Param::new(value);
        }
    }

    /// Which look the Creative controls hold: `Some(None)` when neutral, `None` when
    /// they're set some other way.
    pub fn of(color: &ColorCorrection) -> Option<Option<Self>> {
        let current = creative_values(color);
        let matches = |recipe: Recipe| {
            current
                .iter()
                .zip(recipe.values())
                .all(|(param, value)| **param == Param::new(value))
        };
        if matches(NEUTRAL) {
            return Some(None);
        }
        Self::ALL
            .into_iter()
            .find(|look| matches(look.recipe()))
            .map(Some)
    }
}

impl Recipe {
    fn values(&self) -> [i32; 7] {
        let [shadow_blue, shadow_red] = self.shadow_tint;
        let [highlight_blue, highlight_red] = self.highlight_tint;
        [
            self.faded_film,
            self.vibrance,
            self.saturation,
            shadow_blue,
            shadow_red,
            highlight_blue,
            highlight_red,
        ]
    }
}

fn creative_values(color: &ColorCorrection) -> [&Param; 7] {
    let [shadow_blue, shadow_red] = &color.shadow_tint;
    let [highlight_blue, highlight_red] = &color.highlight_tint;
    [
        &color.faded_film,
        &color.vibrance,
        &color.saturation,
        shadow_blue,
        shadow_red,
        highlight_blue,
        highlight_red,
    ]
}

fn creative_params(color: &mut ColorCorrection) -> [&mut Param; 7] {
    let [shadow_blue, shadow_red] = &mut color.shadow_tint;
    let [highlight_blue, highlight_red] = &mut color.highlight_tint;
    [
        &mut color.faded_film,
        &mut color.vibrance,
        &mut color.saturation,
        shadow_blue,
        shadow_red,
        highlight_blue,
        highlight_red,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_look_reads_back_as_itself_and_none_clears_it() {
        let mut color = ColorCorrection::default();
        for look in CreativeLook::ALL {
            CreativeLook::apply(Some(look), &mut color);
            assert_eq!(CreativeLook::of(&color), Some(Some(look)));
        }
        CreativeLook::apply(None, &mut color);
        assert_eq!(color, ColorCorrection::default());
    }
}
