# Edit

A non-linear video editor for Windows 11, written in Rust. It aims at the everyday
feature set of Premiere Pro: a multi-track timeline with frame-accurate editing, effects,
transitions, keyframes, color, audio mixing and export.

## Features

- **Editing:** multi-track timeline; insert, overwrite, three- and four-point edits;
  ripple, roll, slip, slide and dynamic J/K/L trimming; razor and snip at the playhead;
  linked selection, groups, sync locks, nested sequences, subclips and multicam; markers,
  in/out marks, gaps and through edits; full undo history.
- **Playback:** audio-clocked playback with dropped-frame reporting, J/K/L shuttle, loop
  and play-around, scrubbing with sound, render previews, proxies, hardware decoding
  (D3D12VA) with software fallback, and a full-screen player with a seek bar, skip,
  and pitch-kept playback speed.
- **Picture:** GPU compositing in linear light; motion, opacity, blend modes, crop and
  masks with tracking; well over a hundred video effects; Lumetri-style color (basic,
  creative, curves, hue curves, color wheels, HSL secondary, LUTs, vignette) with scopes;
  keying; stabilization; speed changes, time remapping and optical flow.
- **Transitions:** dissolves, wipes, slides, iris, page turns, morph cut and more, with
  default transitions and alignment.
- **Keyframes:** linear, hold, ease, Bezier and auto Bezier, with value and velocity
  graphs, and built-in animation presets.
- **Titles and graphics:** per-clip titles and shapes, text styles, templates, rolling
  and crawling titles, captions with SRT import and export.
- **Audio:** track and clip mixers with automation, submixes and sends, built-in effects,
  VST3 effect plugins with their own editors, loudness metering and normalization, auto
  ducking, remix to length, voiceover recording.
- **Speech:** on-device transcription (Whisper), auto captions and text-based editing.
- **Export:** H.264, HEVC, ProRes, DNxHR, VP9, AV1, GIF and image sequences; hardware
  encoding (NVENC, AMF, Quick Sync) with software fallback; presets including social
  formats; an export queue; EDL and Final Cut Pro XML interchange.
- **Interface:** Simple mode for quick cuts (the default on first run) and Advanced mode
  with workspaces; customizable shortcuts; autosave and crash recovery.

## Building

On Windows 11 (x64):

1. Install Rust with [rustup](https://rustup.rs). The toolchain is pinned in
   `rust-toolchain.toml` and installs itself on the first build.
2. Install the Visual Studio Build Tools with the "Desktop development with C++"
   workload, [CMake](https://cmake.org) and [LLVM](https://llvm.org) (for `libclang`;
   set `LIBCLANG_PATH` to its `bin` folder if it isn't found). The FFmpeg bindings and
   Whisper are built with them.
3. Download a **shared** FFmpeg 8 build for Windows (for example a `-shared` build from
   gyan.dev or BtbN) and set `FFMPEG_DIR` to the folder holding its `include` and `lib`.
   Put its `bin` folder on `PATH`, or copy its DLLs next to `app.exe`, to run.
4. Build and run:

   ```
   cargo run -p app                 # debug
   cargo build --release            # target\release\app.exe
   ```

Release builds open without a console window.

## Where things are kept

Preferences, workspaces, autosaves, cached waveforms, presets, templates and downloaded
speech models live in `%APPDATA%\Edit`. Projects are `.editproj` files wherever you save
them. VST3 plugins are found in `C:\Program Files\Common Files\VST3`.

Speech models are downloaded on first use from Hugging Face (whisper.cpp's `ggml` files);
any model file put in `%APPDATA%\Edit\models` with the expected name works offline.

## Tests and benchmarks

```
cargo test --workspace                          # everything that needs no GPU or media
cargo test --workspace -- --ignored             # also GPU and real FFmpeg tests
cargo clippy --workspace --all-targets -- -D warnings
```

The benchmarks are ignored tests in `crates/app/src/smoke_tests/`. They print a report
and need no screen. Run them in release with `--nocapture`:

```
cargo test --release -p app <name> -- --ignored --nocapture
```

| Name | Measures | Settings |
| --- | --- | --- |
| `export_speed` | Export time, split into decode, compose, read-back and encode | `EDIT_BENCH_CLIPS`, `EDIT_BENCH_SECONDS`, `EDIT_BENCH_DISSOLVES=1`, `EDIT_BENCH_EFFECTS=1`, `EDIT_BENCH_SOFTWARE_DECODE=1` |
| `playback_speed` | Real-time playback through dissolves: dropped frames, decoding behind, frame times | `EDIT_BENCH_CLIPS`, `EDIT_BENCH_EFFECTS=1`, `EDIT_BENCH_SOFTWARE_DECODE=1` |
| `vst3_plugin` | Loading a plugin, processing 10 s of audio, opening and closing its editor | `EDIT_BENCH_VST3` (a `.vst3` bundle) |
| `transcription` | Speech-to-text time and the first lines heard | `EDIT_BENCH_SPEECH_MODEL`, `EDIT_BENCH_CLIPS` (first file), `EDIT_BENCH_SPEECH_LANGUAGE` |
| `large_project` | Saving and opening a large project, frame times while scrolling and zooming the timeline | `EDIT_BENCH_TRACKS` (20), `EDIT_BENCH_TRACK_CLIPS` (1000) |
| `frame_times_with_a_large_sequence` | Frame times while idle, selecting, scrolling and dragging | |
| `a_long_session_levels_off` | Threads, handles and memory over minutes of playing and editing | |
| `random_gestures_never_panic` | Random clicks, drags and keys all over the window | |

`EDIT_BENCH_CLIPS` lists media files separated by `;`. `EDIT_BENCH_EFFECTS=1` gives every
clip a Lumetri grade and a Gaussian blur.

## Known limits

- Windows only. Other platforms build for development but aren't supported.
- The window has the app's icon; `app.exe` itself shows the default icon in Explorer,
  as embedding one needs a resource-compiler build dependency.
- Only VST3 audio effects are hosted: no instruments, CLAP or AU.
- Speech recognition runs on the CPU.
- The working color space is Rec.709; HDR sources are converted into it.
- Playback speed is set from the full-screen player and goes back to normal on leaving.

## Layout

A Cargo workspace; see `CLAUDE.md` for the crates, rules and conventions.
