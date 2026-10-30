use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use libloading::Library;
use vst3::ComPtr;
use vst3::Steinberg::IPluginFactory;

use crate::PluginError;

type GetFactoryFn = unsafe extern "system" fn() -> *mut IPluginFactory;
type ExitFn = unsafe extern "system" fn() -> bool;

#[cfg(windows)]
const ENTRY_SYMBOL: &str = "InitDll";
#[cfg(windows)]
const EXIT_SYMBOL: &str = "ExitDll";
#[cfg(target_os = "linux")]
const ENTRY_SYMBOL: &str = "ModuleEntry";
#[cfg(target_os = "linux")]
const EXIT_SYMBOL: &str = "ModuleExit";

/// A loaded VST3 binary between its entry and exit calls.
struct Module {
    factory: ManuallyDrop<ComPtr<IPluginFactory>>,
    exit: Option<ExitFn>,
    library: ManuallyDrop<Library>,
}

impl Module {
    #[cfg(any(windows, target_os = "linux"))]
    fn load(path: &Path) -> Result<Self, PluginError> {
        let library_error = |error: libloading::Error| PluginError::Library {
            path: path.to_owned(),
            message: error.to_string(),
        };
        // SAFETY: loading a plugin runs its initializers; that foreign code is trusted by
        // the caller, which chose to load it. No other invariant is required.
        let library = unsafe { Library::new(path) }.map_err(library_error)?;
        // SAFETY: the symbol types match the VST3 module ABI for this platform.
        let (exit, get_factory) = unsafe {
            (
                library
                    .get::<ExitFn>(EXIT_SYMBOL)
                    .map(|symbol| *symbol)
                    .ok(),
                library
                    .get::<GetFactoryFn>("GetPluginFactory")
                    .map(|symbol| *symbol)
                    .map_err(|_| PluginError::MissingSymbol("GetPluginFactory"))?,
            )
        };
        let library = enter(library)?;
        // SAFETY: the module has been entered and `get_factory` stays valid while
        // `library` is loaded. The returned pointer carries one reference for us.
        let factory = unsafe { ComPtr::from_raw(get_factory()) };
        let Some(factory) = factory else {
            if let Some(exit) = exit {
                // SAFETY: balances the successful entry call; the library is still loaded.
                unsafe { exit() };
            }
            return Err(PluginError::NoFactory);
        };
        Ok(Self {
            factory: ManuallyDrop::new(factory),
            exit,
            library: ManuallyDrop::new(library),
        })
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    fn load(_path: &Path) -> Result<Self, PluginError> {
        Err(PluginError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
fn enter(library: Library) -> Result<Library, PluginError> {
    type InitFn = unsafe extern "system" fn() -> bool;
    // SAFETY: `InitDll` takes no arguments and returns a C++ bool, per the VST3 module ABI.
    let init = unsafe { library.get::<InitFn>(ENTRY_SYMBOL) }.map(|symbol| *symbol);
    match init {
        // SAFETY: called once, right after loading, before any other module function.
        Ok(init) if !unsafe { init() } => Err(PluginError::ModuleEntryFailed),
        _ => Ok(library),
    }
}

#[cfg(target_os = "linux")]
fn enter(library: Library) -> Result<Library, PluginError> {
    use std::ffi::c_void;
    type EntryFn = unsafe extern "system" fn(*mut c_void) -> bool;
    // SAFETY: `ModuleEntry` takes the dlopen handle and returns a C++ bool, per the VST3
    // module ABI.
    let entry = unsafe { library.get::<EntryFn>(ENTRY_SYMBOL) }
        .map(|symbol| *symbol)
        .map_err(|_| PluginError::MissingSymbol(ENTRY_SYMBOL))?;
    let handle = libloading::os::unix::Library::from(library).into_raw();
    // SAFETY: `handle` was just produced by `into_raw` and is reclaimed exactly once.
    let library = Library::from(unsafe { libloading::os::unix::Library::from_raw(handle) });
    // SAFETY: called once, right after loading, before any other module function.
    if unsafe { entry(handle) } {
        Ok(library)
    } else {
        Err(PluginError::ModuleEntryFailed)
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        // SAFETY: neither field is used again. The factory must be released before the
        // module exits, and the library unloaded only after that.
        unsafe { ManuallyDrop::drop(&mut self.factory) };
        if let Some(exit) = self.exit {
            // SAFETY: balances the successful entry call; the library is still loaded.
            unsafe { exit() };
        }
        // SAFETY: see above; nothing from the library is referenced any more.
        unsafe { ManuallyDrop::drop(&mut self.library) };
    }
}

type Registry = HashMap<PathBuf, Arc<Module>>;

fn registry() -> MutexGuard<'static, Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// One user of a module. Every module is entered once and exited when its last user drops;
/// both happen under the registry lock so a reload never overlaps a pending exit.
pub(crate) struct SharedModule {
    path: PathBuf,
    module: ManuallyDrop<Arc<Module>>,
}

impl SharedModule {
    pub(crate) fn open(path: &Path) -> Result<Self, PluginError> {
        let mut modules = registry();
        let module = match modules.get(path) {
            Some(module) => Arc::clone(module),
            None => {
                let module = Arc::new(Module::load(path)?);
                modules.insert(path.to_owned(), Arc::clone(&module));
                module
            }
        };
        Ok(Self {
            path: path.to_owned(),
            module: ManuallyDrop::new(module),
        })
    }

    pub(crate) fn factory(&self) -> &ComPtr<IPluginFactory> {
        &self.module.factory
    }
}

impl Drop for SharedModule {
    fn drop(&mut self) {
        let mut modules = registry();
        // SAFETY: `module` is not used after this point.
        let module = unsafe { ManuallyDrop::take(&mut self.module) };
        let registered_and_ours = 2;
        let last_user = if Arc::strong_count(&module) == registered_and_ours {
            modules.remove(&self.path)
        } else {
            None
        };
        drop(module);
        drop(last_user);
        drop(modules);
    }
}
