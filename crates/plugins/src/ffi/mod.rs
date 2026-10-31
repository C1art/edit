mod classes;
mod consts;
mod controller;
#[cfg(windows)]
pub(crate) mod editor;
mod host;
mod instance;
mod message;
mod module;
mod params;
mod process;
mod state;
mod stream;
mod strings;

pub(crate) use instance::Instance;
pub(crate) use module::SharedModule;

use vst3::Steinberg::{kResultOk, tresult};

use crate::PluginError;

fn check(result: tresult, method: &'static str) -> Result<(), PluginError> {
    if result == kResultOk {
        Ok(())
    } else {
        Err(PluginError::Call { method, result })
    }
}
