//! Hosts VST3 audio-effect plugins: scanning, loading, processing, parameters and state.
//!
//! # Threads
//!
//! Everything here runs foreign code, so nothing may be called on the UI thread, which
//! must never block. The VST3 spec assigns calls to threads as follows:
//!
//! - audio thread: [`Plugin::process`];
//! - main thread: everything else, including [`Plugin::load`], parameters, state,
//!   [`Plugin::reset`] and dropping the plugin.
//!
//! A [`Plugin`] is `Send` and not `Sync`, so all its calls are serialized on whichever
//! thread owns it. Plugins generally tolerate that while no editor is open. Once an
//! editor exists, controller calls must move to the thread that owns the editor window.

mod error;
mod ffi;
mod plugin;
mod scan;
mod state;
mod types;

pub use error::PluginError;
pub use plugin::Plugin;
pub use scan::{default_folders, scan};
pub use types::{ClassId, ParamId, ParameterFlags, ParameterInfo, PluginInfo, Tail};
