use serde::{Deserialize, Serialize};

use crate::Param;

/// Premiere's Dynamics: an auto gate, an expander, a compressor and a limiter, run in
/// that order, each on while present. Levels are in decibels and times in milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Dynamics {
    pub gate: Option<GateSection>,
    pub expander: Option<ExpanderSection>,
    pub compressor: Option<CompressorSection>,
    pub limiter: Option<LimiterSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GateSection {
    pub threshold: Param,
    pub attack: Param,
    pub release: Param,
}

/// Pushes what falls under the threshold further down, each decibel under becoming
/// `ratio`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExpanderSection {
    pub threshold: Param,
    pub ratio: Param,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CompressorSection {
    pub threshold: Param,
    pub ratio: Param,
    pub attack: Param,
    pub release: Param,
    pub makeup: Param,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LimiterSection {
    pub threshold: Param,
    pub release: Param,
}

impl Dynamics {
    pub(super) fn params_mut(&mut self) -> Vec<&mut Param> {
        let gate = self
            .gate
            .iter_mut()
            .flat_map(|gate| [&mut gate.threshold, &mut gate.attack, &mut gate.release]);
        let expander = (self.expander.iter_mut())
            .flat_map(|expander| [&mut expander.threshold, &mut expander.ratio]);
        let compressor = self.compressor.iter_mut().flat_map(|compressor| {
            [
                &mut compressor.threshold,
                &mut compressor.ratio,
                &mut compressor.attack,
                &mut compressor.release,
                &mut compressor.makeup,
            ]
        });
        let limiter = (self.limiter.iter_mut())
            .flat_map(|limiter| [&mut limiter.threshold, &mut limiter.release]);
        gate.chain(expander)
            .chain(compressor)
            .chain(limiter)
            .collect()
    }
}

impl Default for Dynamics {
    fn default() -> Self {
        Self {
            gate: None,
            expander: None,
            compressor: Some(CompressorSection::default()),
            limiter: None,
        }
    }
}

impl Default for GateSection {
    fn default() -> Self {
        Self {
            threshold: Param::from_value(-50.0),
            attack: Param::from_value(2.0),
            release: Param::from_value(100.0),
        }
    }
}

impl Default for ExpanderSection {
    fn default() -> Self {
        Self {
            threshold: Param::from_value(-40.0),
            ratio: Param::from_value(2.0),
        }
    }
}

impl Default for CompressorSection {
    fn default() -> Self {
        Self {
            threshold: Param::from_value(-20.0),
            ratio: Param::from_value(3.0),
            attack: Param::from_value(10.0),
            release: Param::from_value(100.0),
            makeup: Param::default(),
        }
    }
}

impl Default for LimiterSection {
    fn default() -> Self {
        Self {
            threshold: Param::from_value(-0.1),
            release: Param::from_value(50.0),
        }
    }
}
