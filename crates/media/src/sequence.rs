//! Numbered picture sequences, which a media path names with an FFmpeg pattern such as
//! `shot_%04d.png` in place of one file.

use std::fs;
use std::path::{Path, PathBuf};

use crate::ffi::InputContext;
use crate::MediaError;

/// A file name split around its frame number, as `{prefix}{number}{suffix}`.
#[derive(Debug, PartialEq, Eq)]
struct Pattern {
    prefix: String,
    /// The digits every number is padded to with zeros, or `None` for unpadded numbers.
    width: Option<usize>,
    suffix: String,
}

impl Pattern {
    fn parse(name: &str) -> Option<Self> {
        let mut prefix = String::new();
        let mut chars = name.chars().peekable();
        while let Some(char) = chars.next() {
            if char != '%' {
                prefix.push(char);
                continue;
            }
            if chars.peek() == Some(&'%') {
                chars.next();
                prefix.push('%');
                continue;
            }
            let mut digits = String::new();
            while let Some(digit) = chars.next_if(char::is_ascii_digit) {
                digits.push(digit);
            }
            if chars.next() != Some('d') {
                return None;
            }
            let width = match digits.as_str() {
                "" => None,
                padded => Some(padded.strip_prefix('0')?.parse().ok()?),
            };
            let suffix: String = chars.collect();
            return (!suffix.contains('%') || suffix.contains("%%")).then(|| Self {
                prefix,
                width,
                suffix: suffix.replace("%%", "%"),
            });
        }
        None
    }

    fn number(&self, name: &str) -> Option<u64> {
        let digits = name
            .strip_prefix(&self.prefix)?
            .strip_suffix(&self.suffix)?;
        let fits = match self.width {
            Some(width) => {
                digits.len() == width || (digits.len() > width && !digits.starts_with('0'))
            }
            None => digits == "0" || !digits.starts_with('0'),
        };
        let all_digits = !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit());
        (fits && all_digits).then(|| digits.parse().ok()).flatten()
    }

    /// The width of the zero-padded numbers among `names` that fit this pattern's
    /// prefix and suffix.
    fn padding_among<'a>(&self, names: impl IntoIterator<Item = &'a str>) -> Option<usize> {
        names.into_iter().find_map(|name| {
            let digits = name
                .strip_prefix(&self.prefix)?
                .strip_suffix(&self.suffix)?;
            let padded = digits.len() > 1 && digits.starts_with('0');
            (padded && digits.bytes().all(|byte| byte.is_ascii_digit())).then_some(digits.len())
        })
    }

    fn name(&self) -> String {
        let number = match self.width {
            Some(width) => format!("%0{width}d"),
            None => "%d".to_owned(),
        };
        let escape = |part: &str| part.replace('%', "%%");
        format!("{}{number}{}", escape(&self.prefix), escape(&self.suffix))
    }
}

/// The pattern naming every frame numbered like `frame`, from the last run of digits in
/// its stem, or `None` when the stem holds no number.
pub fn sequence_pattern(frame: &Path) -> Option<PathBuf> {
    let name = frame.file_name()?.to_str()?;
    let stem_end = frame
        .extension()
        .map_or(name.len(), |extension| name.len() - extension.len() - 1);
    let digits_end = name[..stem_end].rfind(|char: char| char.is_ascii_digit())? + 1;
    let digits_start = name[..digits_end]
        .rfind(|char: char| !char.is_ascii_digit())
        .map_or(0, |index| index + 1);
    let digits = &name[digits_start..digits_end];
    let pattern = Pattern {
        prefix: name[..digits_start].to_owned(),
        width: (digits.starts_with('0') && digits.len() > 1).then_some(digits.len()),
        suffix: name[digits_end..].to_owned(),
    };
    Some(frame.with_file_name(pattern.name()))
}

/// `pattern` padded to the width its folder's frames use, when it was named from a
/// frame whose number needs no padding, such as `shot_100.png` among `shot_001.png`,
/// and would otherwise leave out every padded frame.
pub fn padded_pattern(pattern: &Path) -> PathBuf {
    let padded = || {
        let parsed = Pattern::parse(pattern.file_name()?.to_str()?)?;
        if parsed.width.is_some() || pattern.is_file() {
            return None;
        }
        let folder = pattern
            .parent()
            .filter(|folder| !folder.as_os_str().is_empty());
        let entries = fs::read_dir(folder.unwrap_or(Path::new("."))).ok()?;
        let names: Vec<_> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        let width = parsed.padding_among(names.iter().map(String::as_str))?;
        Some(
            pattern.with_file_name(
                Pattern {
                    width: Some(width),
                    ..parsed
                }
                .name(),
            ),
        )
    };
    padded().unwrap_or_else(|| pattern.to_owned())
}

/// The numbered files `path` names, lowest first, when it is a sequence pattern rather
/// than a file.
pub fn sequence_frames(path: &Path) -> Option<Vec<(u64, PathBuf)>> {
    if path.is_file() {
        return None;
    }
    let pattern = Pattern::parse(path.file_name()?.to_str()?)?;
    let folder = path
        .parent()
        .filter(|folder| !folder.as_os_str().is_empty());
    let entries = fs::read_dir(folder.unwrap_or(Path::new("."))).ok()?;
    let mut frames: Vec<_> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let number = pattern.number(entry.file_name().to_str()?)?;
            Some((number, entry.path()))
        })
        .collect();
    frames.sort_unstable();
    Some(frames)
}

/// The file a sequence pattern names for frame `number`.
pub fn sequence_frame(pattern: &Path, number: u64) -> Option<PathBuf> {
    let parsed = Pattern::parse(pattern.file_name()?.to_str()?)?;
    let width = parsed.width.unwrap_or(0);
    let name = format!("{}{number:0width$}{}", parsed.prefix, parsed.suffix);
    Some(pattern.with_file_name(name))
}

/// Every file a media path reads: the frames of a sequence, or the one file.
pub fn source_files(path: &Path) -> Vec<PathBuf> {
    sequence_frames(path).map_or_else(
        || vec![path.to_owned()],
        |frames| frames.into_iter().map(|(_, file)| file).collect(),
    )
}

/// Whether a media path still leads to a file, or to the first frame of its sequence.
pub fn source_exists(path: &Path) -> bool {
    path.try_exists().unwrap_or(false)
        || sequence_frames(path).is_some_and(|frames| !frames.is_empty())
}

/// Opens a file, or a sequence from its lowest-numbered frame, saying which it was.
pub(crate) fn open_input(path: &Path) -> Result<(InputContext, bool), MediaError> {
    match sequence_frames(path).and_then(|frames| frames.first().map(|(start, _)| *start)) {
        Some(start) => Ok((InputContext::open_sequence(path, start)?, true)),
        None => Ok((InputContext::open(path)?, false)),
    }
}

#[cfg(test)]
mod tests {
    use timeline::{Ticks, Timebase};

    use super::*;
    use crate::{write_still, DecodedFrame, MediaReader, StillFormat};

    #[test]
    #[ignore = "encodes and decodes real files with FFmpeg"]
    fn a_pattern_reads_its_frames_in_order_from_the_lowest_number() {
        let folder = std::env::temp_dir().join("edit-sequence-test");
        fs::create_dir_all(&folder).unwrap();
        let shades = [10, 70, 130, 190];
        for (number, shade) in (1001..).zip(shades) {
            let frame = folder.join(format!("shot_{number}.png"));
            write_still(&frame, StillFormat::Png, (2, 2), &[shade; 16]).unwrap();
        }
        let pattern = sequence_pattern(&folder.join("shot_1001.png")).unwrap();
        let mut reader = MediaReader::open(&pattern, Timebase::HZ_90K).unwrap();
        let info = reader.info().clone();
        let mut read = Vec::new();
        while let Some(frame) = reader.next_frame().unwrap() {
            if let DecodedFrame::Video(frame) = frame {
                read.push(frame.pixels[0]);
            }
        }
        fs::remove_dir_all(&folder).unwrap();
        assert!(!info.still);
        // FFmpeg reads a sequence at 25 frames a second.
        assert_eq!(info.duration, Some(Ticks(4 * 90_000 / 25)));
        assert_eq!(read, shades);
    }

    #[test]
    fn a_frame_names_the_pattern_of_its_sequence() {
        let patterns = [
            ("shot_0001.png", "shot_%04d.png"),
            ("shot_1001.png", "shot_%d.png"),
            ("take2_frame7.tif", "take2_frame%d.tif"),
            ("100%_0010.exr", "100%%_%04d.exr"),
            ("0005", "%04d"),
        ];
        for (frame, pattern) in patterns {
            let named = sequence_pattern(Path::new(frame)).unwrap();
            assert_eq!(named, Path::new(pattern), "{frame}");
        }
        assert_eq!(sequence_pattern(Path::new("still.png")), None);
    }

    #[test]
    fn a_pattern_numbers_only_the_files_it_names() {
        let padded = Pattern::parse("shot_%04d.png").unwrap();
        assert_eq!(padded.number("shot_0042.png"), Some(42));
        assert_eq!(padded.number("shot_12345.png"), Some(12345));
        assert_eq!(padded.number("shot_042.png"), None);
        assert_eq!(padded.number("shot_00042.png"), None);
        assert_eq!(padded.number("shot_0042.jpg"), None);

        let unpadded = Pattern::parse("100%%_%d").unwrap();
        assert_eq!(unpadded.number("100%_7"), Some(7));
        assert_eq!(unpadded.number("100%_07"), None);
        assert_eq!(Pattern::parse("100%_done.png"), None);

        let named_from_100 = Pattern::parse("shot_%d.png").unwrap();
        let folder = ["shot_100.png", "shot_099.png", "shot_1.png", "take_007.png"];
        assert_eq!(named_from_100.padding_among(folder), Some(3));
        assert_eq!(
            named_from_100.padding_among(["shot_7.png", "shot_10.png"]),
            None
        );

        let renamed = Path::new("copies/shot_%04d (2).png");
        let frame = sequence_frame(renamed, 7).unwrap();
        assert_eq!(frame, Path::new("copies/shot_0007 (2).png"));
    }
}
