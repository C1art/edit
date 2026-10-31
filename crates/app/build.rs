//! Gives the Windows exe its icon and version details, which Explorer, the taskbar
//! before the window opens, and Task Manager read from the exe's resources.

use std::error::Error;
use std::path::Path;

#[path = "src/icon_art.rs"]
mod icon_art;

const ICON_SIZES: [u32; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=src/icon_art.rs");
    if std::env::var("CARGO_CFG_TARGET_OS")? != "windows" {
        return Ok(());
    }
    let out_dir = std::env::var("OUT_DIR")?;
    let icon_path = Path::new(&out_dir).join("cledit.ico");
    std::fs::write(&icon_path, ico())?;
    let resource_path = Path::new(&out_dir).join("cledit.rc");
    std::fs::write(&resource_path, resource_script(&icon_path)?)?;
    match embed_resource::compile(&resource_path, embed_resource::NONE) {
        embed_resource::CompilationResult::NotAttempted(reason) => {
            println!("cargo:warning=the exe gets no icon: {reason}");
            Ok(())
        }
        result => Ok(result.manifest_optional()?),
    }
}

/// An .ico of uncompressed 32-bit images, which every Windows version reads at every size.
fn ico() -> Vec<u8> {
    let images: Vec<Vec<u8>> = ICON_SIZES.iter().map(|&size| ico_bitmap(size)).collect();
    let directory_len = 6 + 16 * images.len();
    let mut ico = Vec::new();
    ico.extend(0u16.to_le_bytes());
    ico.extend(1u16.to_le_bytes());
    ico.extend((images.len() as u16).to_le_bytes());
    let mut offset = directory_len;
    for (&size, image) in ICON_SIZES.iter().zip(&images) {
        // 256 doesn't fit the byte, so the format writes it as 0.
        let side = if size >= 256 { 0 } else { size as u8 };
        ico.extend([side, side, 0, 0]);
        ico.extend(1u16.to_le_bytes());
        ico.extend(32u16.to_le_bytes());
        ico.extend((image.len() as u32).to_le_bytes());
        ico.extend((offset as u32).to_le_bytes());
        offset += image.len();
    }
    images.iter().for_each(|image| ico.extend(image));
    ico
}

/// A bitmap header, the colour rows bottom-up in BGRA, then the 1-bit mask Windows still
/// expects after them, left clear since the alpha channel does its job.
fn ico_bitmap(size: u32) -> Vec<u8> {
    let mask_row_len = size.div_ceil(32) * 4;
    let mut bitmap = Vec::new();
    bitmap.extend(40u32.to_le_bytes());
    bitmap.extend((size as i32).to_le_bytes());
    // The height counts the mask's rows as well as the colour rows.
    bitmap.extend((2 * size as i32).to_le_bytes());
    bitmap.extend(1u16.to_le_bytes());
    bitmap.extend(32u16.to_le_bytes());
    bitmap.extend([0; 24]);
    let rgba = icon_art::rgba(size);
    for row in rgba.chunks_exact(size as usize * 4).rev() {
        for &[red, green, blue, alpha] in row.as_chunks::<4>().0 {
            bitmap.extend([blue, green, red, alpha]);
        }
    }
    bitmap.resize(bitmap.len() + (mask_row_len * size) as usize, 0);
    bitmap
}

fn resource_script(icon_path: &Path) -> Result<String, Box<dyn Error>> {
    let icon_path = icon_path.to_str().ok_or("OUT_DIR isn't valid UTF-8")?;
    let version = std::env::var("CARGO_PKG_VERSION")?;
    let numeric_version = version
        .split(['-', '+'])
        .next()
        .unwrap_or_default()
        .replace('.', ",");
    Ok(format!(
        r#"1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {numeric_version},0
PRODUCTVERSION {numeric_version},0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "FileDescription", "Cledit"
      VALUE "ProductName", "Cledit"
      VALUE "InternalName", "cledit"
      VALUE "OriginalFilename", "cledit.exe"
      VALUE "FileVersion", "{version}"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        icon = icon_path.replace('\\', "\\\\"),
    ))
}
