//! Updates an installed Cledit through the same Velopack calls the app makes, without its
//! window, so the release build can check that an installed earlier version updates to the
//! new one. It has to run from the installed `current` folder, where Velopack finds the
//! install: `update_probe <expected version> [feed folder]`. Without a feed folder it updates
//! from the GitHub releases, like the app.

use std::error::Error;
use std::process::ExitCode;

use velopack::sources::{FileSource, GithubSource, UpdateSource};
use velopack::{UpdateCheck, UpdateManager};

const RELEASES: &str = "https://github.com/C1art/edit";

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
    let expected = args
        .next()
        .ok_or("usage: update_probe <expected version> [feed folder]")?;
    let source: Box<dyn UpdateSource> = match args.next() {
        Some(feed) => Box::new(FileSource::new(feed)),
        None => Box::new(GithubSource::new(RELEASES, None, false)),
    };
    let manager = UpdateManager::new_boxed(source, None, None)?;
    println!("installed: {}", manager.get_current_version_as_string());
    let info = match manager.check_for_updates()? {
        UpdateCheck::UpdateAvailable(info) => info,
        UpdateCheck::NoUpdateAvailable => return Err("no update offered".into()),
        UpdateCheck::RemoteIsEmpty => return Err("the release feed is empty".into()),
    };
    let offered = &info.TargetFullRelease.Version;
    if *offered != expected {
        return Err(format!("offered {offered}, expected {expected}").into());
    }
    manager.download_updates(&info, None)?;
    println!("downloaded: {}", info.TargetFullRelease.FileName);
    manager.wait_exit_then_apply_updates(
        &info.TargetFullRelease,
        true,
        false,
        Vec::<String>::new(),
    )?;
    println!("applying {offered} once this exits");
    Ok(())
}
