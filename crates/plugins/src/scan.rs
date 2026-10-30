use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::ffi::SharedModule;
use crate::{PluginError, PluginInfo};

const AUDIO_EFFECT_CATEGORY: &str = "Audio Module Class";
const EFFECT_SUBCATEGORY: &str = "Fx";
/// Guards against symlink cycles inside plugin folders.
const MAX_FOLDER_DEPTH: usize = 8;

#[cfg(all(windows, target_arch = "x86_64"))]
const BUNDLE_ARCH: &str = "x86_64-win";
#[cfg(all(windows, target_arch = "aarch64"))]
const BUNDLE_ARCH: &str = "arm64-win";
#[cfg(all(windows, target_arch = "x86"))]
const BUNDLE_ARCH: &str = "x86-win";
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const BUNDLE_ARCH: &str = "x86_64-linux";
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const BUNDLE_ARCH: &str = "aarch64-linux";
#[cfg(all(target_os = "linux", target_arch = "x86"))]
const BUNDLE_ARCH: &str = "i386-linux";
#[cfg(not(any(windows, target_os = "linux")))]
const BUNDLE_ARCH: &str = "";

#[cfg(windows)]
const BINARY_EXTENSION: &str = "vst3";
#[cfg(not(windows))]
const BINARY_EXTENSION: &str = "so";

/// The standard VST3 folders for this OS, whether or not they exist.
#[cfg(windows)]
pub fn default_folders() -> Vec<PathBuf> {
    let under = |variable: &str, relative: &str| {
        std::env::var_os(variable).map(|base| PathBuf::from(base).join(relative))
    };
    [
        under("CommonProgramFiles", "VST3"),
        under("LOCALAPPDATA", r"Programs\Common\VST3"),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The standard VST3 folders for this OS, whether or not they exist.
#[cfg(target_os = "linux")]
pub fn default_folders() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".vst3"));
    home.into_iter()
        .chain([
            PathBuf::from("/usr/lib/vst3"),
            PathBuf::from("/usr/local/lib/vst3"),
        ])
        .collect()
}

/// Hosting is only implemented for Windows and Linux.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn default_folders() -> Vec<PathBuf> {
    Vec::new()
}

/// Lists the audio-effect classes of every `.vst3` bundle under `folders`.
///
/// This loads and runs code from every plugin it finds, so it can take long and must
/// never run on the UI thread. Bundles that fail to load are skipped, but a plugin that
/// crashes while loading takes the whole process down; scan in a child process if that
/// matters.
pub fn scan(folders: &[PathBuf]) -> Vec<PluginInfo> {
    let mut bundles = Vec::new();
    for folder in folders {
        collect_bundles(folder, MAX_FOLDER_DEPTH, &mut bundles);
    }
    bundles.sort();
    bundles.dedup();
    bundles
        .iter()
        .flat_map(|bundle| effect_classes(bundle))
        .collect()
}

fn collect_bundles(folder: &Path, depth: usize, bundles: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if is_bundle_name(&path) {
            bundles.push(path);
        } else if depth > 0 && path.is_dir() {
            collect_bundles(&path, depth - 1, bundles);
        }
    }
}

fn is_bundle_name(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("vst3"))
}

fn effect_classes(bundle: &Path) -> Vec<PluginInfo> {
    let Ok(module) = bundle_binary(bundle).and_then(|binary| SharedModule::open(&binary)) else {
        return Vec::new();
    };
    module
        .classes()
        .into_iter()
        .filter(|class| {
            class.category == AUDIO_EFFECT_CATEGORY
                && class
                    .subcategories
                    .iter()
                    .any(|sub| sub == EFFECT_SUBCATEGORY)
        })
        .map(|class| PluginInfo {
            path: bundle.to_owned(),
            class_id: class.class_id,
            name: class.name,
            vendor: class.vendor,
            version: class.version,
            category: class.category,
            subcategories: class.subcategories,
        })
        .collect()
}

/// Resolves a bundle directory to its binary for this platform. A file is returned as
/// is, covering Windows' legacy single-file `.vst3` DLLs.
pub(crate) fn bundle_binary(bundle: &Path) -> Result<PathBuf, PluginError> {
    if bundle.is_file() {
        return Ok(bundle.to_owned());
    }
    let not_found = || PluginError::BundleBinaryNotFound(bundle.to_owned());
    let mut file_name = bundle.file_stem().ok_or_else(not_found)?.to_os_string();
    file_name.push(".");
    file_name.push(BINARY_EXTENSION);
    let binary = bundle.join("Contents").join(BUNDLE_ARCH).join(file_name);
    if binary.is_file() {
        Ok(binary)
    } else {
        Err(not_found())
    }
}
