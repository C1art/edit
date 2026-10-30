use std::fmt;
use std::path::PathBuf;

use crate::ParamId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    /// Only Windows and Linux module loading is implemented.
    UnsupportedPlatform,
    BundleBinaryNotFound(PathBuf),
    Library {
        path: PathBuf,
        message: String,
    },
    MissingSymbol(&'static str),
    /// The module's entry function (`InitDll` / `ModuleEntry`) returned false.
    ModuleEntryFailed,
    NoFactory,
    ClassNotFound,
    MissingInterface(&'static str),
    /// A plugin method returned a failing `tresult`.
    Call {
        method: &'static str,
        result: i32,
    },
    /// The plugin cannot be configured with a stereo main input and output.
    UnsupportedBusLayout,
    UnsupportedSampleSize,
    InvalidProcessSetup,
    /// All input and output channel slices passed to `process` must have the same length.
    BufferLengthMismatch,
    UnknownParameter(ParamId),
    InvalidParameterValue,
    InvalidState,
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => f.write_str("VST3 hosting is not supported on this OS"),
            Self::BundleBinaryNotFound(path) => {
                write!(f, "no plugin binary inside bundle {}", path.display())
            }
            Self::Library { path, message } => {
                write!(f, "cannot load {}: {message}", path.display())
            }
            Self::MissingSymbol(name) => write!(f, "plugin module does not export {name}"),
            Self::ModuleEntryFailed => f.write_str("plugin module entry function failed"),
            Self::NoFactory => f.write_str("plugin module returned no factory"),
            Self::ClassNotFound => f.write_str("plugin factory cannot create the class"),
            Self::MissingInterface(name) => write!(f, "plugin does not implement {name}"),
            Self::Call { method, result } => write!(f, "{method} failed with result {result}"),
            Self::UnsupportedBusLayout => f.write_str("plugin does not accept stereo in/out"),
            Self::UnsupportedSampleSize => f.write_str("plugin cannot process 32-bit floats"),
            Self::InvalidProcessSetup => {
                f.write_str("sample rate and maximum block size must be non-zero")
            }
            Self::BufferLengthMismatch => {
                f.write_str("input and output buffers have different lengths")
            }
            Self::UnknownParameter(id) => write!(f, "unknown parameter {}", id.0),
            Self::InvalidParameterValue => f.write_str("parameter value is not finite"),
            Self::InvalidState => f.write_str("plugin state blob is malformed"),
        }
    }
}

impl std::error::Error for PluginError {}
