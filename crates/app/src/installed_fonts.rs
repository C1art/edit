use std::path::{Path, PathBuf};
use std::sync::{Once, OnceLock};

static SCAN: Once = Once::new();
static FONTS: OnceLock<Vec<InstalledFont>> = OnceLock::new();

pub(crate) struct InstalledFont {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
}

/// The TrueType and OpenType files installed for everyone and for this user, sorted
/// by name; `None` until the folders have been read, which starts on the first call.
pub(crate) fn list() -> Option<&'static [InstalledFont]> {
    SCAN.call_once(|| {
        let scan = std::thread::Builder::new()
            .name("font scan".into())
            .spawn(|| {
                let _ = FONTS.set(scan());
            });
        if scan.is_err() {
            let _ = FONTS.set(Vec::new());
        }
    });
    FONTS.get().map(Vec::as_slice)
}

/// The installed font at `path`'s full name, as the list shows it.
pub(crate) fn name_of(path: &Path) -> Option<&'static str> {
    list()?
        .iter()
        .find(|font| font.path == path)
        .map(|font| font.name.as_str())
}

fn scan() -> Vec<InstalledFont> {
    let windows = std::env::var_os("WINDIR").map(|dir| Path::new(&dir).join("Fonts"));
    let user = std::env::var_os("LOCALAPPDATA").map(|dir| {
        Path::new(&dir)
            .join("Microsoft")
            .join("Windows")
            .join("Fonts")
    });
    let mut fonts: Vec<InstalledFont> = [windows, user]
        .into_iter()
        .flatten()
        .filter_map(|folder| std::fs::read_dir(folder).ok())
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    ["ttf", "otf", "ttc"].contains(&extension.to_ascii_lowercase().as_str())
                })
        })
        .filter_map(|path| {
            let name = full_name(&std::fs::read(&path).ok()?)?;
            Some(InstalledFont { name, path })
        })
        .collect();
    fonts.sort_by_cached_key(|font| font.name.to_lowercase());
    fonts.dedup_by(|a, b| a.name == b.name);
    fonts
}

/// The full font name, as "Arial Bold", from the first font's naming table, in
/// English where the font has it.
fn full_name(data: &[u8]) -> Option<String> {
    const FULL_NAME: u16 = 4;
    const WINDOWS: u16 = 3;
    const UNICODE_BMP: u16 = 1;
    const ENGLISH_US: u16 = 0x0409;
    let short = |at: usize| Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?));
    let long = |at: usize| Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?));
    let directory = if data.starts_with(b"ttcf") {
        long(12)? as usize
    } else {
        0
    };
    let table = (0..usize::from(short(directory + 4)?))
        .map(|index| directory + 12 + index * 16)
        .find(|&record| data.get(record..record + 4) == Some(b"name"))?;
    let names = long(table + 8)? as usize;
    let strings = names + usize::from(short(names + 4)?);
    let records = (0..usize::from(short(names + 2)?)).map(|index| names + 6 + index * 12);
    let full_names: Vec<(u16, usize, usize)> = records
        .filter_map(|record| {
            let platform = short(record)?;
            let encoding = short(record + 2)?;
            let wanted =
                platform == WINDOWS && encoding == UNICODE_BMP && short(record + 6)? == FULL_NAME;
            wanted.then_some((
                short(record + 4)?,
                usize::from(short(record + 8)?),
                strings + usize::from(short(record + 10)?),
            ))
        })
        .collect();
    let (_, length, offset) = full_names
        .iter()
        .find(|(language, ..)| *language == ENGLISH_US)
        .or(full_names.first())?;
    let utf16: Vec<u16> = data
        .get(*offset..offset + length)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_be_bytes(pair))
        .collect();
    let name = String::from_utf16(&utf16).ok()?;
    (!name.trim().is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_english_full_name_is_read_from_the_naming_table() {
        let utf16 =
            |text: &str| -> Vec<u8> { text.encode_utf16().flat_map(u16::to_be_bytes).collect() };
        let (german, english) = (utf16("Beispiel Fett"), utf16("Example Bold"));
        let record = |language: u16, name: u16, length: usize, offset: usize| {
            [3, 1, language, name, length as u16, offset as u16]
                .into_iter()
                .flat_map(u16::to_be_bytes)
                .collect::<Vec<u8>>()
        };
        // A table directory with one table, then the naming table it points at.
        let mut font = vec![0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        font.extend(b"name");
        font.extend([0; 4]);
        font.extend(28u32.to_be_bytes());
        font.extend([0; 4]);
        let records = [
            record(0x0407, 4, german.len(), 0),
            record(0x0409, 1, english.len(), german.len()),
            record(0x0409, 4, english.len(), german.len()),
        ];
        let strings = 6 + records.len() * 12;
        font.extend([0, 0, 0, records.len() as u8, 0, strings as u8]);
        font.extend(records.concat());
        font.extend(german);
        font.extend(english);
        assert_eq!(full_name(&font).as_deref(), Some("Example Bold"));
        assert_eq!(full_name(&font[..40]), None);
    }
}
