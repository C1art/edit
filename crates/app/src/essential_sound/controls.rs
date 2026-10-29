use std::ops::RangeInclusive;

use timeline::{AudioEffect, Param};

const AMOUNT: RangeInclusive<f32> = 0.0..=10.0;
/// Where a control starts when it is switched on.
const DEFAULT_AMOUNT: f32 = 5.0;

/// The voice equalizers: name, then low, mid and high in dB at full amount, and the mid
/// frequency that tells them apart.
const EQUALIZERS: [(&str, [f32; 3], f32); 4] = [
    ("Subtle Voice Enhancer", [-2.0, 2.0, 3.0], 2_500.0),
    ("Podcast Voice", [-4.0, 4.0, 2.0], 3_000.0),
    ("Warm Voice", [3.0, -2.0, -1.0], 800.0),
    ("Reduce Boominess", [-8.0, -2.0, 0.0], 250.0),
];

/// The reverb rooms: name, room size and damping in percent.
const ROOMS: [(&str, f32, f32); 4] = [
    ("Small Room", 25.0, 60.0),
    ("Large Room", 60.0, 40.0),
    ("Auditorium", 85.0, 30.0),
    ("Outside", 45.0, 85.0),
];

const MAINS: [(&str, f32); 2] = [("50 Hz", 50.0), ("60 Hz", 60.0)];

/// One of Essential Sound's switches and sliders, standing for one effect in the clip's
/// chain whose settings follow a single amount, as Premiere's do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Control {
    ReduceNoise,
    ReduceRumble,
    DeHum,
    DeEss,
    Dynamics,
    Equalize,
    Reverb,
    Pan,
}

impl Control {
    /// In signal-chain order: repair before clarity before space, so one switched on
    /// later still lands where it belongs.
    const ALL: [Self; 8] = [
        Self::ReduceNoise,
        Self::ReduceRumble,
        Self::DeHum,
        Self::DeEss,
        Self::Dynamics,
        Self::Equalize,
        Self::Reverb,
        Self::Pan,
    ];

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::ReduceNoise => "Reduce Noise",
            Self::ReduceRumble => "Reduce Rumble",
            Self::DeHum => "DeHum",
            Self::DeEss => "DeEss",
            Self::Dynamics => "Dynamics",
            Self::Equalize => "EQ",
            Self::Reverb => "Reverb",
            Self::Pan => "Pan",
        }
    }

    pub(super) fn tip(self) -> &'static str {
        match self {
            Self::ReduceNoise => "Lowers steady background noise such as hiss and room tone",
            Self::ReduceRumble => "Cuts low rumble such as wind, handling and traffic",
            Self::DeHum => "Removes mains hum and its harmonics",
            Self::DeEss => "Softens harsh S sounds",
            Self::Dynamics => "Evens out loud and quiet words",
            Self::Equalize => "Shapes the voice's tone",
            Self::Reverb => "Places the sound in a room",
            Self::Pan => "Places the sound left or right",
        }
    }

    pub(super) fn range(self) -> RangeInclusive<f32> {
        match self {
            Self::Pan => -100.0..=100.0,
            _ => AMOUNT,
        }
    }

    /// The names of the presets the control chooses among; empty when it has none.
    pub(super) fn variants(self) -> Vec<&'static str> {
        match self {
            Self::DeHum => MAINS.iter().map(|(name, _)| *name).collect(),
            Self::Equalize => EQUALIZERS.iter().map(|(name, ..)| *name).collect(),
            Self::Reverb => ROOMS.iter().map(|(name, ..)| *name).collect(),
            _ => Vec::new(),
        }
    }

    fn owns(self, effect: &AudioEffect) -> bool {
        matches!(
            (self, effect),
            (Self::ReduceNoise, AudioEffect::DeNoise { .. })
                | (Self::ReduceRumble, AudioEffect::HighPass { .. })
                | (Self::DeHum, AudioEffect::DeHum { .. })
                | (Self::DeEss, AudioEffect::DeEsser { .. })
                | (Self::Dynamics, AudioEffect::Compressor { .. })
                | (Self::Equalize, AudioEffect::Equalizer { .. })
                | (Self::Reverb, AudioEffect::Reverb { .. })
                | (Self::Pan, AudioEffect::Balance { .. })
        )
    }

    /// The control's amount and preset in `effects`, or `None` while it is off.
    pub(super) fn setting(self, effects: &[AudioEffect]) -> Option<(f32, usize)> {
        let effect = effects.iter().find(|effect| self.owns(effect))?;
        let setting = match effect {
            AudioEffect::DeNoise { amount } => (amount.value() / 10.0, 0),
            AudioEffect::HighPass { frequency } => ((frequency.value() - 40.0) / 12.0, 0),
            AudioEffect::DeHum {
                frequency,
                reduction,
                ..
            } => (
                reduction.value() / 4.0,
                nearest(&[50.0, 60.0], frequency.value()),
            ),
            AudioEffect::DeEsser { reduction, .. } => (reduction.value() / 2.0, 0),
            AudioEffect::Compressor { ratio, .. } => ((ratio.value() - 1.0) / 0.5, 0),
            AudioEffect::Equalizer {
                low,
                mid,
                mid_frequency,
                high,
            } => {
                let frequencies = EQUALIZERS.map(|(_, _, frequency)| frequency);
                let preset = nearest(&frequencies, mid_frequency.value());
                let gains = EQUALIZERS[preset].1;
                let (index, full) = gains
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
                    .map_or((0, 1.0), |(index, full)| (index, *full));
                let value = [low, mid, high][index].value();
                (value / full * 10.0, preset)
            }
            AudioEffect::Reverb { size, mix, .. } => {
                let sizes = ROOMS.map(|(_, size, _)| size);
                (mix.value() / 5.0, nearest(&sizes, size.value()))
            }
            AudioEffect::Balance { balance } => (balance.value(), 0),
            _ => return None,
        };
        let range = self.range();
        Some((setting.0.clamp(*range.start(), *range.end()), setting.1))
    }

    /// Sets the control in `effects`, adding its effect if it is off, or removes the
    /// effect for `None`. Other effects are left alone.
    pub(super) fn set(self, effects: &mut Vec<AudioEffect>, setting: Option<(f32, usize)>) {
        let found = effects.iter().position(|effect| self.owns(effect));
        match (setting, found) {
            (None, Some(index)) => {
                effects.remove(index);
            }
            (None, None) => {}
            (Some(setting), Some(index)) => effects[index] = self.effect(setting),
            (Some(setting), None) => {
                let later = Self::ALL
                    .iter()
                    .skip_while(|&&control| control != self)
                    .skip(1);
                let before = effects
                    .iter()
                    .position(|effect| later.clone().any(|control| control.owns(effect)));
                effects.insert(before.unwrap_or(effects.len()), self.effect(setting));
            }
        }
    }

    pub(super) fn switched_on(self) -> (f32, usize) {
        match self {
            Self::Pan => (0.0, 0),
            _ => (DEFAULT_AMOUNT, 0),
        }
    }

    fn effect(self, (amount, variant): (f32, usize)) -> AudioEffect {
        let value = Param::from_value;
        match self {
            Self::ReduceNoise => AudioEffect::DeNoise {
                amount: value(amount * 10.0),
            },
            Self::ReduceRumble => AudioEffect::HighPass {
                frequency: value(40.0 + amount * 12.0),
            },
            Self::DeHum => AudioEffect::DeHum {
                frequency: value(MAINS[variant.min(MAINS.len() - 1)].1),
                harmonics: value(4.0),
                reduction: value(amount * 4.0),
            },
            Self::DeEss => AudioEffect::DeEsser {
                threshold: value(-10.0 - amount * 3.0),
                reduction: value(amount * 2.0),
                frequency: value(6_000.0),
            },
            Self::Dynamics => AudioEffect::Compressor {
                threshold: value(-6.0 - amount * 2.4),
                ratio: value(1.0 + amount * 0.5),
                attack: value(5.0),
                release: value(120.0),
                makeup: value(amount * 0.8),
            },
            Self::Equalize => {
                let (_, [low, mid, high], mid_frequency) =
                    EQUALIZERS[variant.min(EQUALIZERS.len() - 1)];
                let scaled = |full: f32| value(full * amount / 10.0);
                AudioEffect::Equalizer {
                    low: scaled(low),
                    mid: scaled(mid),
                    mid_frequency: value(mid_frequency),
                    high: scaled(high),
                }
            }
            Self::Reverb => {
                let (_, size, damping) = ROOMS[variant.min(ROOMS.len() - 1)];
                AudioEffect::Reverb {
                    size: value(size),
                    damping: value(damping),
                    mix: value(amount * 5.0),
                }
            }
            Self::Pan => AudioEffect::Balance {
                balance: value(amount),
            },
        }
    }
}

fn nearest(candidates: &[f32], value: f32) -> usize {
    candidates
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - value).abs().total_cmp(&(b.1 - value).abs()))
        .map_or(0, |(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_reads_back_as_set() {
        for control in Control::ALL {
            let variants = control.variants().len().max(1);
            for variant in 0..variants {
                let amount = if control == Control::Pan { -30.0 } else { 7.5 };
                let mut effects = Vec::new();
                control.set(&mut effects, Some((amount, variant)));
                let (read, read_variant) = control.setting(&effects).unwrap();
                assert!((read - amount).abs() < 1e-4, "{control:?} read {read}");
                assert_eq!(read_variant, variant, "{control:?}");
            }
        }
    }

    #[test]
    fn switching_on_lands_in_chain_order_and_off_leaves_the_rest() {
        let delay = AudioEffect::catalog()
            .into_iter()
            .find(|effect| matches!(effect, AudioEffect::Delay { .. }))
            .unwrap();
        let mut effects = vec![delay.clone()];
        Control::Reverb.set(&mut effects, Some((5.0, 0)));
        Control::ReduceNoise.set(&mut effects, Some((5.0, 0)));
        Control::Dynamics.set(&mut effects, Some((5.0, 0)));
        let order: Vec<_> = effects.iter().map(AudioEffect::name).collect();
        assert_eq!(
            order,
            ["Delay", "DeNoise", "Dynamics Compressor", "Studio Reverb"]
        );
        Control::ReduceNoise.set(&mut effects, None);
        Control::Reverb.set(&mut effects, None);
        assert_eq!(effects.len(), 2);
        assert_eq!(effects[0], delay);
    }
}
