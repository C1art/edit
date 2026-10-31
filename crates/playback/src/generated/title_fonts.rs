use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::epaint::text::{FontData, FontDefinitions, Fonts, TextOptions};
use egui::epaint::{FontColorTransferFunction, FontFamily};
use timeline::TitleFont;

/// Font atlases only grow, so a long session of title edits starts a fresh one here.
const ATLAS_REFILL: f32 = 0.8;

/// The fonts titles are set in: egui's own, plus every font file a title has named.
pub(super) struct TitleFonts {
    fonts: Fonts,
    definitions: FontDefinitions,
    /// The family each named file is set as, or `None` when it could not be read.
    files: HashMap<PathBuf, Option<FontFamily>>,
}

impl TitleFonts {
    pub(super) fn new() -> Self {
        let definitions = FontDefinitions::default();
        Self {
            fonts: new_fonts(definitions.clone()),
            definitions,
            files: HashMap::new(),
        }
    }

    /// The family `font` is set in, with its file read first if this is the first
    /// title to name it. Unreadable files fall back to the sans serif.
    pub(super) fn family(&mut self, font: &TitleFont) -> FontFamily {
        match font {
            TitleFont::Sans => FontFamily::Proportional,
            TitleFont::Mono => FontFamily::Monospace,
            TitleFont::File(path) => {
                if !self.files.contains_key(path) {
                    let family = self.install(path);
                    self.files.insert(path.clone(), family);
                }
                self.files
                    .get(path)
                    .cloned()
                    .flatten()
                    .unwrap_or(FontFamily::Proportional)
            }
        }
    }

    /// Starts a fresh atlas when the old one is nearly full; call only before laying
    /// out, as it drops the glyphs earlier layouts point into.
    pub(super) fn make_room(&mut self) {
        if self.fonts.font_atlas_fill_ratio() >= ATLAS_REFILL {
            self.fonts = new_fonts(self.definitions.clone());
        }
    }

    pub(super) fn fonts(&mut self) -> &mut Fonts {
        &mut self.fonts
    }

    fn install(&mut self, path: &Path) -> Option<FontFamily> {
        let bytes = std::fs::read(path).ok()?;
        if !is_font_file(&bytes) {
            return None;
        }
        let name = path.to_string_lossy().into_owned();
        self.definitions
            .font_data
            .insert(name.clone(), Arc::new(FontData::from_owned(bytes)));
        // egui's own fonts stand in for any letter the file lacks.
        let mut chain = vec![name.clone()];
        chain.extend(
            self.definitions
                .families
                .get(&FontFamily::Proportional)
                .into_iter()
                .flatten()
                .cloned(),
        );
        let family = FontFamily::Name(name.into());
        self.definitions.families.insert(family.clone(), chain);
        self.fonts = new_fonts(self.definitions.clone());
        Some(family)
    }
}

fn new_fonts(definitions: FontDefinitions) -> Fonts {
    let options = TextOptions {
        max_texture_side: 8192,
        color_transfer_function: FontColorTransferFunction::Gamma(1.0),
        subpixel_binning: false,
        ..TextOptions::default()
    };
    Fonts::new(options, definitions)
}

/// Whether `data` opens with a TrueType or OpenType table directory, or a collection
/// pointing at one. egui panics on a font it cannot parse, and the table directory is
/// all its parser insists on.
fn is_font_file(data: &[u8]) -> bool {
    let word = |at: usize| -> Option<u32> {
        let bytes = data.get(at..at.checked_add(4)?)?;
        Some(u32::from_be_bytes(bytes.try_into().ok()?))
    };
    let directory = if data.starts_with(b"ttcf") {
        let fonts = word(8).unwrap_or(0);
        match word(12) {
            Some(first) if fonts > 0 => first as usize,
            _ => return false,
        }
    } else {
        0
    };
    let versions = [
        0x0001_0000,
        u32::from_be_bytes(*b"OTTO"),
        u32::from_be_bytes(*b"true"),
    ];
    let Some(version) = word(directory) else {
        return false;
    };
    let Some(tables) = word(directory + 4).map(|word| (word >> 16) as usize) else {
        return false;
    };
    versions.contains(&version) && directory + 12 + tables * 16 <= data.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_files_with_a_table_directory_pass_as_fonts() {
        let directory = |version: &[u8; 4], tables: u16| {
            let mut file = version.to_vec();
            file.extend(tables.to_be_bytes());
            file.extend([0; 6]);
            file.extend(vec![0; usize::from(tables) * 16]);
            file
        };
        assert!(is_font_file(&directory(&[0, 1, 0, 0], 2)));
        assert!(is_font_file(&directory(b"OTTO", 0)));
        assert!(!is_font_file(&directory(b"wOFF", 0)));
        assert!(!is_font_file(b"not a font"));

        let mut short = directory(b"true", 3);
        short.pop();
        assert!(!is_font_file(&short));

        let mut collection = b"ttcf\0\x01\0\0\0\0\0\x01\0\0\0\x10".to_vec();
        collection.extend(directory(b"OTTO", 1));
        assert!(is_font_file(&collection));
        collection.truncate(16);
        assert!(!is_font_file(&collection));
    }

    #[test]
    fn a_font_file_is_set_as_its_own_family_and_a_bad_one_as_sans() {
        let folder = std::env::temp_dir().join(format!("title-fonts-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let defaults = FontDefinitions::default();
        let (_, data) = defaults.font_data.iter().next().unwrap();
        let good = folder.join("good.ttf");
        let bytes: &[u8] = (*data.font).as_ref();
        std::fs::write(&good, bytes).unwrap();
        let bad = folder.join("bad.ttf");
        std::fs::write(&bad, b"not a font").unwrap();

        let mut fonts = TitleFonts::new();
        let family = fonts.family(&TitleFont::File(good));
        assert!(matches!(family, FontFamily::Name(_)));
        assert_eq!(
            fonts.family(&TitleFont::File(bad)),
            FontFamily::Proportional
        );
        assert!(fonts
            .fonts()
            .with_pixels_per_point(1.0)
            .has_glyph(&egui::FontId::new(20.0, family), 'A'));
        std::fs::remove_dir_all(folder).unwrap();
    }
}
