//! The plugins the last scan found, so starting up doesn't have to load every plugin.

use std::path::PathBuf;

use plugins::{ClassId, PluginInfo};

fn path() -> PathBuf {
    crate::project_store::app_folder().join("Plugins.txt")
}

/// One plugin per line: class, bundle, name, vendor, version and category, tab-separated.
pub(super) fn read() -> Vec<PluginInfo> {
    let Ok(text) = std::fs::read_to_string(path()) else {
        return Vec::new();
    };
    text.lines().filter_map(parse).collect()
}

pub(super) fn write(installed: &[PluginInfo]) {
    let lines: String = installed
        .iter()
        .map(|info| {
            let class: String = info
                .class_id
                .0
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            format!(
                "{class}\t{}\t{}\t{}\t{}\t{}\n",
                info.path.display(),
                info.name,
                info.vendor,
                info.version,
                info.category
            )
        })
        .collect();
    // Without the list, the next start just shows no plugins until a scan.
    let _ = std::fs::create_dir_all(crate::project_store::app_folder());
    let _ = std::fs::write(path(), lines);
}

fn parse(line: &str) -> Option<PluginInfo> {
    let mut fields = line.split('\t');
    let class = fields.next()?;
    let mut class_id = [0; 16];
    for (byte, pair) in class_id.iter_mut().zip(class.as_bytes().chunks(2)) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(PluginInfo {
        class_id: ClassId(class_id),
        path: PathBuf::from(fields.next()?),
        name: fields.next()?.to_owned(),
        vendor: fields.next()?.to_owned(),
        version: fields.next()?.to_owned(),
        category: fields.next()?.to_owned(),
        subcategories: Vec::new(),
    })
}
