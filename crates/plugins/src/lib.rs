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
//! thread. Plugins generally tolerate that while no editor is open, so an
//! [`EditorWindow`] runs a separate instance on the thread that owns its window.

pub mod editor;
mod error;
mod ffi;
mod plugin;
mod scan;
mod state;
mod types;

pub use editor::{EditorEvent, EditorSetup, EditorWindow};
pub use error::PluginError;
pub use plugin::Plugin;
pub use scan::{bundle_effects, default_folders, scan};
pub use types::{ClassId, ParamId, ParameterFlags, ParameterInfo, PluginInfo, Tail};
