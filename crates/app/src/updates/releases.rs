//! The newest Cledit on GitHub and its installer: found, downloaded, checked and run.
//! `examples/update_probe.rs` includes this file to update the way the app does, so it
//! uses nothing else of the app's.

use std::fmt::Write as _;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use serde::Deserialize;
use sha2::{Digest, Sha256};

const LATEST: &str = "https://api.github.com/repos/C1art/edit/releases/latest";
const USER_AGENT: &str = concat!("Cledit/", env!("CARGO_PKG_VERSION"));
/// Left next to the installed exe by the installer; a build folder has none.
const UNINSTALLER: &str = "unins000.exe";

#[derive(Debug, Clone, Default)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub installer: Installer,
}

#[derive(Debug, Clone, Default)]
pub struct Installer {
    name: String,
    url: String,
    size: u64,
    sha256: Option<String>,
}

#[derive(Deserialize)]
struct ReleaseJson {
    tag_name: String,
    body: Option<String>,
    assets: Vec<AssetJson>,
}

#[derive(Deserialize)]
struct AssetJson {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

pub fn installed() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join(UNINSTALLER)))
        .is_some_and(|uninstaller| uninstaller.is_file())
}

pub fn latest() -> Result<Release, String> {
    let json = ureq::get(LATEST)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|error| error.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|error| error.to_string())?;
    parse(&json)
}

fn parse(json: &str) -> Result<Release, String> {
    let release: ReleaseJson = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let version = release.tag_name.trim_start_matches('v').to_owned();
    let wanted = installer_name(&version);
    let asset = release
        .assets
        .into_iter()
        .find(|asset| asset.name == wanted)
        .ok_or_else(|| format!("Cledit {version} has no installer to download"))?;
    Ok(Release {
        version,
        notes: release.body.unwrap_or_default(),
        installer: Installer {
            sha256: asset
                .digest
                .and_then(|digest| Some(digest.strip_prefix("sha256:")?.to_owned())),
            name: asset.name,
            url: asset.browser_download_url,
            size: asset.size,
        },
    })
}

fn installer_name(version: &str) -> String {
    format!("Cledit-{version}-Setup.exe")
}

/// Whether `candidate` comes after `current`. Only plain `major.minor.patch` versions
/// count, so a pre-release is never offered.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (numbers(candidate), numbers(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

fn numbers(version: &str) -> Option<[u64; 3]> {
    let mut parts = version.split('.').map(|part| part.parse().ok());
    let numbers = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(numbers)
}

/// Into the temporary folder, reporting whole percents, and checked against the size
/// and SHA-256 that GitHub lists for it.
pub fn download(installer: &Installer, mut percent: impl FnMut(u8)) -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join(&installer.name);
    let downloaded = download_to(installer, &path, &mut percent);
    if downloaded.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    downloaded.map(|()| path)
}

fn download_to(
    installer: &Installer,
    path: &Path,
    percent: &mut impl FnMut(u8),
) -> Result<(), String> {
    let mut response = ureq::get(&installer.url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| error.to_string())?;
    let mut body = response.body_mut().as_reader();
    let mut file = File::create(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut chunk = vec![0; 1 << 16];
    let mut received = 0u64;
    let mut reported = None;
    loop {
        let read = body.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        file.write_all(&chunk[..read])
            .map_err(|error| error.to_string())?;
        hasher.update(&chunk[..read]);
        received += read as u64;
        let now = whole_percent(received, installer.size);
        if reported != Some(now) {
            reported = Some(now);
            percent(now);
        }
    }
    file.sync_all().map_err(|error| error.to_string())?;
    if received != installer.size {
        return Err(format!(
            "got {received} bytes of {}, so the download is incomplete",
            installer.size
        ));
    }
    let Some(expected) = &installer.sha256 else {
        return Ok(());
    };
    let found = hasher
        .finalize()
        .iter()
        .fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        });
    if !found.eq_ignore_ascii_case(expected) {
        return Err("the download is damaged: its checksum doesn't match".to_owned());
    }
    Ok(())
}

fn whole_percent(done: u64, total: u64) -> u8 {
    u8::try_from(done.saturating_mul(100) / total.max(1)).unwrap_or(100)
}

/// Updates the installed Cledit: the installer asks nothing and shows only its progress,
/// keeping the folder and choices of the install it replaces. With `relaunch`, it starts
/// Cledit again when done.
pub fn install(installer: &Path, relaunch: bool) -> std::io::Result<Child> {
    let mut command = Command::new(installer);
    command.args(["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]);
    if relaunch {
        command.arg("/relaunch=1");
    }
    command.spawn()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions_compare_by_number_not_text() {
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
    }

    #[test]
    fn pre_releases_and_odd_versions_are_never_newer() {
        assert!(!is_newer("0.3.0-beta", "0.2.0"));
        assert!(!is_newer("0.3", "0.2.0"));
        assert!(!is_newer("0.3.0.1", "0.2.0"));
    }

    #[test]
    fn a_release_offers_its_own_installer_with_its_checksum() {
        let json = r#"{
            "tag_name": "v0.2.0",
            "body": "Notes",
            "assets": [
                {"name": "Cledit-0.1.2-Setup.exe", "browser_download_url": "old", "size": 1, "digest": null},
                {"name": "Cledit-0.2.0-Setup.exe", "browser_download_url": "new", "size": 7, "digest": "sha256:AB12"}
            ]
        }"#;
        let release = parse(json).unwrap();
        assert_eq!(release.version, "0.2.0");
        assert_eq!(release.installer.url, "new");
        assert_eq!(release.installer.sha256.as_deref(), Some("AB12"));
    }

    #[test]
    fn a_release_without_an_installer_is_an_error() {
        let json = r#"{"tag_name": "v0.1.2", "body": null, "assets": [
            {"name": "Cledit-win-Setup.exe", "browser_download_url": "x", "size": 1}
        ]}"#;
        assert!(parse(json).is_err());
    }
}
