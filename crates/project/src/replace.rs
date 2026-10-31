use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// How long to wait for another program to let go of the file before saving fails.
const IN_USE_WAITS: [Duration; 5] = [
    Duration::from_millis(20),
    Duration::from_millis(50),
    Duration::from_millis(100),
    Duration::from_millis(250),
    Duration::from_millis(500),
];

/// Writes beside `path` first and then swaps it in, so a crash or failed write never
/// leaves a half-written file where the last good one was.
pub fn replace_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    refuse_read_only(path)?;
    // Unique per process and call, so two editors or threads saving the same file never
    // write into one partial file.
    static SAVES: AtomicU64 = AtomicU64::new(0);
    let save = SAVES.fetch_add(1, Ordering::Relaxed);
    let mut partial = path.as_os_str().to_owned();
    partial.push(format!(".{}-{save}.saving", std::process::id()));
    let partial = Path::new(&partial);
    let written = File::create(partial).and_then(|mut file| {
        file.write_all(contents)?;
        // Without this a power cut can keep the rename yet lose the contents it names,
        // leaving an empty file.
        file.sync_all()
    });
    written
        .and_then(|()| retry_while_in_use(|| std::fs::rename(partial, path)))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(partial);
        })
}

/// Renames a finished file over `path`, as `replace_file` does once it has written it.
pub fn rename_into_place(finished: &Path, path: &Path) -> io::Result<()> {
    refuse_read_only(path)?;
    retry_while_in_use(|| std::fs::rename(finished, path))
}

/// Windows won't rename over a read-only file, and only says access is denied.
fn refuse_read_only(path: &Path) -> io::Result<()> {
    let read_only = cfg!(windows)
        && path
            .metadata()
            .is_ok_and(|metadata| metadata.permissions().readonly());
    if read_only {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} is read-only", path.display()),
        ));
    }
    Ok(())
}

/// Tries again for a moment while a file is in use, as on Windows a virus scanner,
/// sync client, file preview or our own player opening it briefly is enough to make
/// a rename or removal fail.
pub fn retry_while_in_use<T>(mut attempt: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut waits = IN_USE_WAITS.iter();
    loop {
        match attempt() {
            Err(error) if is_in_use(&error) => match waits.next() {
                Some(&wait) => std::thread::sleep(wait),
                None => return Err(error),
            },
            done => return done,
        }
    }
}

fn is_in_use(error: &io::Error) -> bool {
    const ERROR_ACCESS_DENIED: i32 = 5;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    cfg!(windows)
        && matches!(
            error.raw_os_error(),
            Some(ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION)
        )
}

#[cfg(all(test, windows))]
mod tests {
    use std::os::windows::fs::OpenOptionsExt;

    use super::*;

    // Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
    #[allow(clippy::unwrap_used)]
    fn scratch(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("edit-{name}-{}.editproj", std::process::id()));
        std::fs::write(&path, b"old").unwrap();
        path
    }

    #[test]
    fn a_save_waits_for_another_program_to_close_the_file() {
        const FILE_SHARE_READ: u32 = 1;
        let path = scratch("held");
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&path)
            .unwrap();
        let other = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(60));
            drop(held);
        });
        replace_file(&path, b"new").unwrap();
        other.join().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_read_only_file_is_left_alone_and_named_as_such() {
        let path = scratch("read-only");
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&path, permissions.clone()).unwrap();
        let error = replace_file(&path, b"new").unwrap_err();
        assert!(error.to_string().ends_with("is read-only"), "{error}");
        assert_eq!(std::fs::read(&path).unwrap(), b"old");
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        std::fs::set_permissions(&path, permissions).unwrap();
        std::fs::remove_file(&path).unwrap();
    }
}
