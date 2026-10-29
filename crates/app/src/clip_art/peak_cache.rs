use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use media::Peaks;

/// Bumped whenever the layout changes, so older files read as missing.
const MAGIC: &[u8; 8] = b"EDPEAKS1";
/// Peak files of media not opened for this long are removed.
const KEEP_UNUSED: Duration = Duration::from_secs(60 * 60 * 24 * 90);

/// What identifies one version of a media file: a peak file only stands for the media
/// while its path, size and modification time all match.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    path: String,
    size: u64,
    modified_ns: u64,
}

impl Stamp {
    fn of(media: &Path) -> Option<Self> {
        let metadata = fs::metadata(media).ok()?;
        let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        Some(Self {
            path: media.to_string_lossy().into_owned(),
            size: metadata.len(),
            modified_ns: u64::try_from(modified.as_nanos()).ok()?,
        })
    }

    /// A name that stays the same across runs and builds, unlike `std`'s hasher.
    fn file_name(&self) -> String {
        let hash = self
            .path
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
            });
        format!("{hash:016x}.peaks")
    }
}

/// Peaks kept on disk between sessions, as Premiere's media cache keeps them, so a
/// project reopens without decoding all of its audio again. Every call touches the disk,
/// so use it from a worker thread.
#[derive(Debug)]
pub(super) struct PeakCache {
    folder: PathBuf,
}

impl PeakCache {
    pub(super) fn new(folder: PathBuf) -> Self {
        Self { folder }
    }

    pub(super) fn read(&self, media: &Path) -> Option<Peaks> {
        let stamp = Stamp::of(media)?;
        let entry = self.folder.join(stamp.file_name());
        let peaks = decode(&fs::read(&entry).ok()?, &stamp)?;
        // Marks the file as used, so clearing out old ones keeps it.
        if let Ok(file) = fs::File::options().append(true).open(&entry) {
            let _ = file.set_modified(SystemTime::now());
        }
        Some(peaks)
    }

    /// Best effort: a peak file that can't be written only means decoding again next time.
    pub(super) fn write(&self, media: &Path, peaks: &Peaks) {
        let Some(stamp) = Stamp::of(media) else {
            return;
        };
        if fs::create_dir_all(&self.folder).is_err() {
            return;
        }
        let entry = self.folder.join(stamp.file_name());
        // Written aside and moved into place, so a crash never leaves half a file.
        let partial = entry.with_extension("partial");
        if fs::write(&partial, encode(&stamp, peaks)).is_ok() {
            let _ = fs::rename(&partial, &entry);
        }
    }

    /// Removes peak files nothing has read for a long time.
    pub(super) fn clear_unused(&self) {
        let Ok(entries) = fs::read_dir(&self.folder) else {
            return;
        };
        for entry in entries.flatten() {
            let unused_for = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| modified.elapsed().ok());
            if unused_for.is_some_and(|unused_for| unused_for > KEEP_UNUSED) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

fn encode(stamp: &Stamp, peaks: &Peaks) -> Vec<u8> {
    let path = stamp.path.as_bytes();
    let buckets = peaks.buckets();
    let mut bytes = Vec::with_capacity(MAGIC.len() + 24 + path.len() + buckets.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&stamp.size.to_le_bytes());
    bytes.extend_from_slice(&stamp.modified_ns.to_le_bytes());
    bytes.extend_from_slice(&(path.len() as u64).to_le_bytes());
    bytes.extend_from_slice(path);
    bytes.extend_from_slice(buckets);
    bytes
}

/// The peaks in `bytes`, if they were made from the media `stamp` describes.
fn decode(bytes: &[u8], stamp: &Stamp) -> Option<Peaks> {
    let rest = bytes.strip_prefix(MAGIC)?;
    let (size, rest) = read_u64(rest)?;
    let (modified_ns, rest) = read_u64(rest)?;
    let (path_len, rest) = read_u64(rest)?;
    let path_len = usize::try_from(path_len).ok()?;
    let (path, buckets) = (rest.get(..path_len)?, rest.get(path_len..)?);
    let matches =
        size == stamp.size && modified_ns == stamp.modified_ns && path == stamp.path.as_bytes();
    matches.then(|| Peaks::from_buckets(buckets.to_vec()))
}

fn read_u64(bytes: &[u8]) -> Option<(u64, &[u8])> {
    let (head, rest) = bytes.split_first_chunk::<8>()?;
    Some((u64::from_le_bytes(*head), rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp() -> Stamp {
        Stamp {
            path: r"D:\Footage\interview.wav".to_owned(),
            size: 123_456_789,
            modified_ns: 1_700_000_000_123_456_789,
        }
    }

    #[test]
    fn peaks_round_trip_through_a_file() {
        let peaks = Peaks::from_buckets(vec![0, 12, 255, 7, 99]);
        assert_eq!(decode(&encode(&stamp(), &peaks), &stamp()), Some(peaks));
    }

    #[test]
    fn a_changed_or_moved_file_misses() {
        let bytes = encode(&stamp(), &Peaks::from_buckets(vec![1, 2, 3]));
        let changed = Stamp {
            modified_ns: stamp().modified_ns + 1,
            ..stamp()
        };
        let resized = Stamp { size: 1, ..stamp() };
        let moved = Stamp {
            path: r"D:\Footage\other.wav".to_owned(),
            ..stamp()
        };
        for other in [changed, resized, moved] {
            assert_eq!(decode(&bytes, &other), None);
        }
        assert_eq!(decode(&bytes[..20], &stamp()), None);
    }
}
