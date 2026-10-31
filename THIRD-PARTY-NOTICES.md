# Third-party notices

Cledit is licensed under the GNU General Public License v3.0 or later (see `LICENSE`).
It includes or links the following software.

## FFmpeg

The installer ships FFmpeg's shared libraries (`av*.dll`, `sw*.dll`)
from a GPL build by [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds), which
includes x264, x265 and other GPL components. FFmpeg is licensed under the GNU GPL
version 3 or later in this configuration.

- FFmpeg source: <https://github.com/FFmpeg/FFmpeg> at the version given below
- Build scripts and the source of every bundled library:
  <https://github.com/BtbN/FFmpeg-Builds>

The exact FFmpeg version of a release is listed at the end of this file in the installed
copy.

## whisper.cpp

Speech to text uses [whisper.cpp](https://github.com/ggml-org/whisper.cpp) and ggml,
compiled into `cledit.exe`. MIT License, Copyright (c) 2023-2024 The ggml authors.

## Rust crates

Cledit is built from many Rust crates published on crates.io, including egui, eframe,
wgpu, cpal, rubato, crossbeam, serde, vst3, whisper-rs, windows and velopack. They are
licensed under MIT, Apache-2.0, BSD, Zlib or similarly permissive terms; each crate's
license is in its source at <https://crates.io>. `ffmpeg-sys-next` is under the WTFPL.

## Microsoft Visual C++ Runtime

The installer installs the Visual C++ 2015-2022 Redistributable from Microsoft when it
is missing, under Microsoft's own license terms.
