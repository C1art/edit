//! A minimal VST3 stereo gain effect used by the `plugins` crate's integration tests.
//!
//! It exposes a combined class (component and controller in one object) and a split
//! class (separate controller connected through `IConnectionPoint`), with one parameter,
//! "Gain": normalized 0..1 maps to 0..2x linear, default 0.5. The component state is the
//! normalized gain as a little-endian f64.
//!
//! The module entry and exit functions refuse unbalanced calls and the factory refuses
//! to exist outside them, so a host that gets the module lifecycle wrong fails to load.

mod combined;
mod controller;
mod delegate;
mod factory;
mod processor;
mod split;
mod strings;

use std::sync::atomic::{AtomicBool, Ordering};

use vst3::ComWrapper;
use vst3::Steinberg::IPluginFactory;

static ENTERED: AtomicBool = AtomicBool::new(false);

fn enter() -> bool {
    !ENTERED.swap(true, Ordering::SeqCst)
}

fn exit() -> bool {
    ENTERED.swap(false, Ordering::SeqCst)
}

/// Runs when the library is unloaded (or the process exits with it loaded). Aborting
/// makes a host that unloads the module without calling `ModuleExit` fail its tests.
#[cfg(target_os = "linux")]
#[used]
#[unsafe(link_section = ".fini_array")]
static CHECK_EXITED_ON_UNLOAD: extern "C" fn() = check_exited_on_unload;

#[cfg(target_os = "linux")]
extern "C" fn check_exited_on_unload() {
    if ENTERED.load(Ordering::SeqCst) {
        eprintln!("plugin_fixture unloaded without ModuleExit");
        std::process::abort();
    }
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn GetPluginFactory() -> *mut IPluginFactory {
    if !ENTERED.load(Ordering::SeqCst) {
        return std::ptr::null_mut();
    }
    ComWrapper::new(factory::Factory)
        .to_com_ptr::<IPluginFactory>()
        .map_or(std::ptr::null_mut(), |factory| factory.into_raw())
}

#[cfg(target_os = "linux")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn ModuleEntry(_library_handle: *mut std::ffi::c_void) -> bool {
    enter()
}

#[cfg(target_os = "linux")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn ModuleExit() -> bool {
    exit()
}

#[cfg(windows)]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn InitDll() -> bool {
    enter()
}

#[cfg(windows)]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "system" fn ExitDll() -> bool {
    exit()
}
