use std::path::PathBuf;

/// A VST3 class ID in the byte order the plugin's factory reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClassId(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ParamId(pub u32);

/// One audio-effect class found by [`scan`](crate::scan).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInfo {
    /// The `.vst3` bundle (or, on Windows, single-file `.vst3` DLL) to pass to
    /// [`Plugin::load`](crate::Plugin::load).
    pub path: PathBuf,
    pub class_id: ClassId,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub category: String,
    pub subcategories: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterInfo {
    pub id: ParamId,
    pub title: String,
    pub short_title: String,
    pub units: String,
    pub default_normalized: f64,
    /// Zero means the parameter is continuous.
    pub step_count: u32,
    pub flags: ParameterFlags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParameterFlags(pub(crate) i32);

impl ParameterFlags {
    const CAN_AUTOMATE: i32 = 1;
    const READ_ONLY: i32 = 1 << 1;
    const LIST: i32 = 1 << 3;
    const HIDDEN: i32 = 1 << 4;
    const BYPASS: i32 = 1 << 16;

    pub fn automatable(self) -> bool {
        self.0 & Self::CAN_AUTOMATE != 0
    }

    pub fn read_only(self) -> bool {
        self.0 & Self::READ_ONLY != 0
    }

    pub fn list(self) -> bool {
        self.0 & Self::LIST != 0
    }

    pub fn hidden(self) -> bool {
        self.0 & Self::HIDDEN != 0
    }

    pub fn bypass(self) -> bool {
        self.0 & Self::BYPASS != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tail {
    Samples(u32),
    Infinite,
}
