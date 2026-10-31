use egui::IconData;

use crate::icon_art;

const SIZE: u32 = 256;

pub(crate) fn icon() -> IconData {
    IconData {
        rgba: icon_art::rgba(SIZE),
        width: SIZE,
        height: SIZE,
    }
}
