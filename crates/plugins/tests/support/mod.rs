use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

#[cfg(target_os = "linux")]
const LIBRARY_NAME: &str = "libplugin_fixture.so";
#[cfg(windows)]
const LIBRARY_NAME: &str = "plugin_fixture.dll";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const BUNDLE_ARCH: &str = "x86_64-linux";
#[cfg(all(windows, target_arch = "x86_64"))]
const BUNDLE_ARCH: &str = "x86_64-win";

#[cfg(target_os = "linux")]
const BINARY_EXTENSION: &str = "so";
#[cfg(windows)]
const BINARY_EXTENSION: &str = "vst3";

/// Builds the fixture with the same profile as this test binary, so a stale library is
/// never tested, and returns its path.
fn fixture_library() -> &'static Path {
    static LIBRARY: OnceLock<PathBuf> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let exe = std::env::current_exe().expect("test executable path");
        let profile_dir = exe
            .parent()
            .and_then(Path::parent)
            .expect("test executable lives in <target>/<profile>/deps");
        let profile = match profile_dir.file_name().and_then(|name| name.to_str()) {
            Some("debug") => "dev",
            Some(other) => other,
            None => panic!("unexpected target layout: {}", profile_dir.display()),
        };
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let status = Command::new(cargo)
            .args([
                "build",
                "--quiet",
                "-p",
                "plugin_fixture",
                "--profile",
                profile,
            ])
            .status()
            .expect("run cargo to build plugin_fixture");
        assert!(status.success(), "building plugin_fixture failed");
        let library = profile_dir.join(LIBRARY_NAME);
        assert!(library.is_file(), "missing {}", library.display());
        library
    })
}

/// A private folder holding a copy of the fixture as `Fixture.vst3`. Each test gets its
/// own copy so the OS loads a separate module, isolating the module lifecycle per test.
pub struct PluginFolder {
    root: PathBuf,
}

impl PluginFolder {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "edit-plugins-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let folder = Self { root };
        let binary = folder.binary_path("Fixture");
        fs::create_dir_all(binary.parent().expect("binary has a parent")).expect("create bundle");
        fs::copy(fixture_library(), &binary).expect("copy fixture into bundle");
        folder
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn fixture_bundle(&self) -> PathBuf {
        self.root.join("Fixture.vst3")
    }

    /// Adds a bundle whose binary is not a loadable library, nested one folder deep.
    pub fn add_broken_bundle(&self) {
        let binary = self.root.join("Vendor").join("Broken.vst3");
        let binary = binary
            .join("Contents")
            .join(BUNDLE_ARCH)
            .join(format!("Broken.{BINARY_EXTENSION}"));
        fs::create_dir_all(binary.parent().expect("binary has a parent")).expect("create bundle");
        fs::write(binary, b"not a shared library").expect("write broken binary");
    }

    fn binary_path(&self, name: &str) -> PathBuf {
        self.root
            .join(format!("{name}.vst3"))
            .join("Contents")
            .join(BUNDLE_ARCH)
            .join(format!("{name}.{BINARY_EXTENSION}"))
    }
}

impl Drop for PluginFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
