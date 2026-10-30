use std::path::Path;

/// Which files a browser shows, by groups of extensions; no groups shows them all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileFilter {
    pub name: &'static str,
    pub extensions: &'static [&'static [&'static str]],
}

const VIDEO: &[&str] = &[
    "mp4", "mov", "mkv", "mxf", "avi", "webm", "m4v", "mts", "m2ts", "ts", "mpg", "mpeg", "wmv",
    "flv", "3gp", "dv", "gif",
];
const AUDIO: &[&str] = &[
    "wav", "mp3", "aac", "m4a", "flac", "ogg", "opus", "aif", "aiff", "wma", "ac3",
];
const STILLS: &[&str] = &[
    "png", "jpg", "jpeg", "tif", "tiff", "bmp", "webp", "dpx", "exr", "tga",
];
pub const ALL_FILES: FileFilter = FileFilter {
    name: "All Files",
    extensions: &[],
};
pub const ALL_MEDIA: FileFilter = FileFilter {
    name: "All Supported Media",
    extensions: &[VIDEO, AUDIO, STILLS, &["xml"]],
};
pub const VIDEO_FILES: FileFilter = FileFilter {
    name: "Video",
    extensions: &[VIDEO],
};
pub const AUDIO_FILES: FileFilter = FileFilter {
    name: "Audio",
    extensions: &[AUDIO],
};
pub const STILL_IMAGES: FileFilter = FileFilter {
    name: "Still Images",
    extensions: &[STILLS],
};
pub const FINAL_CUT_XML: FileFilter = FileFilter {
    name: "Final Cut Pro XML",
    extensions: &[&["xml"]],
};
pub const EDL: FileFilter = FileFilter {
    name: "CMX 3600 EDL",
    extensions: &[&["edl"]],
};
pub const MARKERS_CSV: FileFilter = FileFilter {
    name: "Comma-Separated Values",
    extensions: &[&["csv"]],
};
pub const SUBRIP: FileFilter = FileFilter {
    name: "SubRip Captions",
    extensions: &[&["srt"]],
};
pub const CUBE_LUT: FileFilter = FileFilter {
    name: "Cube LUT",
    extensions: &[&["cube"]],
};
pub const FRAME_IMAGES: FileFilter = FileFilter {
    name: "PNG or TIFF",
    extensions: &[&["png", "tif", "tiff"]],
};
pub const PROJECTS: FileFilter = FileFilter {
    name: "Projects",
    extensions: &[&[project::EXTENSION]],
};

impl FileFilter {
    pub fn shows(self, path: &Path) -> bool {
        self.extensions.is_empty()
            || path.extension().is_some_and(|extension| {
                self.extensions
                    .iter()
                    .flat_map(|group| group.iter())
                    .any(|wanted| extension.eq_ignore_ascii_case(wanted))
            })
    }

    /// What a saved file gets when its name is typed without one.
    pub fn default_extension(self) -> Option<&'static str> {
        self.extensions.first()?.first().copied()
    }
}
