//! Updates an installed Cledit the way the app does, without its window, so the release
//! build can check that an installed earlier version updates to the new one:
//! `update_probe <expected version> [installer]`. Without an installer it downloads the
//! one GitHub offers, like the app.

use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;

#[path = "../src/updates/releases.rs"]
#[allow(dead_code, reason = "the probe skips the app's own install check")]
mod releases;

const USAGE: &str = "usage: update_probe <expected version> [installer]";

fn main() -> ExitCode {
    match update() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("update_probe: {error}");
            ExitCode::FAILURE
        }
    }
}

fn update() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let expected = args.next().ok_or(USAGE)?;
    let installer = match args.next() {
        Some(path) => PathBuf::from(path),
        None => {
            let release = releases::latest()?;
            if release.version != expected {
                return Err(
                    format!("GitHub offers {}, expected {expected}", release.version).into(),
                );
            }
            let installer = releases::download(&release.installer, |_| {})?;
            println!("downloaded and checked {}", installer.display());
            installer
        }
    };
    let finished = releases::install(&installer, false)?.wait()?;
    if !finished.success() {
        return Err(format!("the installer failed: {finished}").into());
    }
    println!("installed {expected}");
    Ok(())
}
