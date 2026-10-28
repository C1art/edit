use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use render::{Grade, Lut3d, CURVE_SIZE};
use timeline::{ColorCorrection, Curves};

use crate::PlaybackError;

/// Grades kept for reuse, about as many as clips with curves or LUTs on screen at once.
const KEPT_GRADES: usize = 16;

type GradeKey = (Curves, Option<PathBuf>, Option<PathBuf>);

thread_local! {
    /// Every LUT asked for, or `None` where it couldn't be read, which was reported once.
    static LUTS: RefCell<Vec<(PathBuf, Option<Arc<Lut3d>>)>> = const { RefCell::new(Vec::new()) };
    /// Most recently used last, so the compositor sees the same `Arc` each frame and
    /// uploads nothing new.
    static GRADES: RefCell<Vec<(GradeKey, Arc<Grade>)>> = const { RefCell::new(Vec::new()) };
}

/// The lookups `color` needs, if any. LUT files are read on first use; one that can't
/// be read is reported in `warnings` and left out.
pub(crate) fn grade_for(
    color: &ColorCorrection,
    warnings: &mut Vec<PlaybackError>,
) -> Option<Arc<Grade>> {
    let curved = !color.curves.is_identity();
    if !curved && color.input_lut.is_none() && color.look.is_none() {
        return None;
    }
    let key = (
        color.curves.clone(),
        color.input_lut.clone(),
        color.look.clone(),
    );
    GRADES.with_borrow_mut(|grades| {
        if let Some(index) = grades.iter().position(|(kept, _)| *kept == key) {
            let entry = grades.remove(index);
            let grade = Arc::clone(&entry.1);
            grades.push(entry);
            return Some(grade);
        }
        let mut lut = |path: &Option<PathBuf>| path.as_deref().and_then(|path| lut(path, warnings));
        let grade = Arc::new(Grade {
            input: lut(&color.input_lut),
            look: lut(&color.look),
            curves: curved.then(|| color.curves.table(CURVE_SIZE)),
        });
        if grades.len() >= KEPT_GRADES {
            grades.remove(0);
        }
        grades.push((key, Arc::clone(&grade)));
        Some(grade)
    })
}

fn lut(path: &Path, warnings: &mut Vec<PlaybackError>) -> Option<Arc<Lut3d>> {
    LUTS.with_borrow_mut(|luts| {
        if let Some((_, lut)) = luts.iter().find(|(kept, _)| kept == path) {
            return lut.clone();
        }
        let lut = std::fs::read_to_string(path)
            .map_err(|error| error.to_string())
            .and_then(|text| Lut3d::parse_cube(&text).map_err(|error| error.to_string()))
            .map(Arc::new)
            .map_err(|reason| {
                warnings.push(PlaybackError::Lut {
                    path: path.to_owned(),
                    reason,
                });
            })
            .ok();
        luts.push((path.to_owned(), lut.clone()));
        lut
    })
}
