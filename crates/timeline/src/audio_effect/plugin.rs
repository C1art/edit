use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Param;

/// A VST3 audio effect. Its parameters start at the plugin's own defaults; only those
/// the user has changed are kept, so a plugin update that adds parameters still loads.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginEffect {
    pub name: String,
    #[serde(default)]
    pub vendor: String,
    /// The `.vst3` bundle, as scanning found it.
    pub path: PathBuf,
    /// Which of the bundle's effects this is.
    pub class_id: [u8; 16],
    #[serde(default)]
    pub params: Vec<PluginParam>,
}

/// A changed plugin parameter, normalized to `0..=1` as VST3 exchanges them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginParam {
    pub id: u32,
    pub value: Param,
}

impl PluginEffect {
    pub fn param(&self, id: u32) -> Option<&Param> {
        self.params
            .iter()
            .find(|param| param.id == id)
            .map(|param| &param.value)
    }

    /// The parameter `id`, added at `default` when it hasn't been changed before.
    pub fn param_mut(&mut self, id: u32, default: f32) -> &mut Param {
        let index = match self.params.iter().position(|param| param.id == id) {
            Some(index) => index,
            None => {
                self.params.push(PluginParam {
                    id,
                    value: Param::from_value(default),
                });
                self.params.len() - 1
            }
        };
        &mut self.params[index].value
    }
}
