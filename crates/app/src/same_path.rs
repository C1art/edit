use std::path::Path;

/// Whether two paths name the same file, telling case apart only where the file
/// system does.
#[cfg(windows)]
pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    let folded = |path: &Path| path.to_string_lossy().replace('/', "\\").to_lowercase();
    folded(a) == folded(b)
}

#[cfg(not(windows))]
pub(crate) fn same_path(a: &Path, b: &Path) -> bool {
    a == b
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn windows_paths_match_whatever_their_case_and_slashes() {
        let same = |a: &str, b: &str| same_path(Path::new(a), Path::new(b));
        assert!(same(r"C:\Out\Cut.MP4", "c:/out/cut.mp4"));
        assert!(same(r"C:\Out\Été.mov", r"C:\Out\éTÉ.mov"));
        assert!(!same(r"C:\Out\Cut.mp4", r"C:\Out\Cut 2.mp4"));
    }
}
