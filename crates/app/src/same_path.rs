use std::path::{Component, Path, PathBuf};

/// Whether two paths name the same file, telling case apart only where the file
/// system does. Compared as written, without asking the disk, as this runs on the UI
/// thread; links and Windows short names aren't followed.
pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    folded(&resolved(a)) == folded(&resolved(b))
}

/// `path` made absolute with `.` and `..` worked out, so `out/../cut.mp4` from the
/// working folder matches the full path of `cut.mp4` there.
fn resolved(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut resolved = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            other => resolved.push(other),
        }
    }
    resolved
}

#[cfg(windows)]
fn folded(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

#[cfg(not(windows))]
fn folded(path: &Path) -> &Path {
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same(a: &str, b: &str) -> bool {
        same_path(Path::new(a), Path::new(b))
    }

    #[test]
    fn relative_and_dotted_paths_match_the_file_they_lead_to() {
        let here = std::env::current_dir().unwrap_or_default();
        let full = here.join("cut.mp4");
        let full = full.to_string_lossy();
        assert!(same("out/../cut.mp4", &full));
        assert!(same("./cut.mp4", &full));
        assert!(!same("out/cut.mp4", &full));
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_match_whatever_their_case_and_slashes() {
        assert!(same(r"C:\Out\Cut.MP4", "c:/out/cut.mp4"));
        assert!(same(r"C:\Out\Été.mov", r"C:\Out\éTÉ.mov"));
        assert!(!same(r"C:\Out\Cut.mp4", r"C:\Out\Cut 2.mp4"));
    }
}
