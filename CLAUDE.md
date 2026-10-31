# CLAUDE.md

## Project

A non-linear video editor for Windows 11, built in Rust. The end goal is feature parity with Premiere Pro: multi-track timeline, frame-accurate editing, effects, transitions, keyframes, color, audio mixing and export.

## Stack

- Rust (stable, edition 2021), target `x86_64-pc-windows-msvc`
- UI: `egui` / `eframe`
- GPU: `wgpu` (DX12 primary, Vulkan fallback) + WGSL shaders
- Media: FFmpeg through our own safe wrapper over `ffmpeg-sys-next`
- Audio: `cpal` for output, `rubato` for resampling
- Concurrency: `crossbeam` (channels, queues)
- Persistence: `serde`
- Windows APIs: `windows` (D3D12/DXGI texture sharing and other Win32 calls), as a `cfg(windows)` dependency
- Speech: `whisper-rs` (whisper.cpp, CPU), as a `cfg(windows)` dependency
- Plugins: `vst3` bindings and `libloading` for hosting VST3 effects, as `cfg(windows)` dependencies
- Exe resources: `embed-resource` (build dependency of `app`) for the exe's icon and version info
- Ask before adding any dependency not listed here.

## Structure

Cargo workspace. Dependencies flow downward only; `timeline` and `project` never depend on `app`, `render` or `media`.

- `crates/app/` — eframe entry point, panels, input, wiring
- `crates/timeline/` — sequences, tracks, clips, edit commands, undo/redo (pure logic, no I/O)
- `crates/media/` — FFmpeg wrapper: demux, decode, encode, probing. All FFmpeg `unsafe` lives in `media/src/ffi/`
- `crates/render/` — wgpu compositor, effects, transitions; shaders in `render/shaders/*.wgsl`
- `crates/audio/` — cpal output stream, rubato resampling, mixer
- `crates/playback/` — engine that schedules decode, render and audio against the master clock
- `crates/project/` — project file format, serde, versioned migrations
- `crates/speech/` — Whisper transcription and model downloads into `%APPDATA%\Cledit\models`
- `crates/plugins/` — VST3 scanning, loading, processing and editor windows
- `crates/plugin_fixture/` — a minimal VST3 `cdylib` that plugin tests load
- `tests/fixtures/` — small media files used by integration tests

## Commands

- Run: `cargo run -p app`
- Release: `cargo build --release`
- Test: `cargo test --workspace`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Format: `cargo fmt --all`
- FFmpeg: set `FFMPEG_DIR` to a shared FFmpeg build (8.1 or newer; the Windows D3D12 decode path needs its headers) before building

## Verification

After every change, in this order:

1. `cargo fmt --all`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test -p <changed crate>` (whole workspace before committing)

## Architecture rules

- **Time:** store all timestamps as integer ticks with a rational timebase. Never use `f64` seconds in the model; convert only at display.
- **Threads:** the UI thread never decodes, does file I/O or waits on a channel. Work goes to worker threads; results come back over `crossbeam` channels and are polled each frame.
- **Audio callback:** no allocation, no locks, no blocking, no logging. Read from a pre-filled lock-free queue only. Resampling happens before the callback, not inside it.
- **Clock:** the audio clock is the master; video follows it and drops or repeats frames to stay in sync.
- **Color:** composite in linear light on `Rgba16Float` textures. Convert from source color space at upload and to display space at present.
- **Edits:** every timeline mutation is an edit command with `apply` and `revert`. Nothing mutates the timeline outside the undo stack.
- **FFmpeg:** wrap every raw pointer in an owning type with `Drop`. Expose only safe APIs outside `media/src/ffi/`. Every `unsafe` block gets a `// SAFETY:` comment stating the invariant.
- **Project files:** carry a format version. New fields use `#[serde(default)]`; breaking changes add a migration in `project`.
- **Errors:** one error enum per crate implementing `std::error::Error`. No `unwrap`/`expect` outside tests unless the invariant is local and obvious.

## Code style

- Follow standard Rust conventions: `rustfmt` defaults, `snake_case` / `CamelCase` / `SCREAMING_SNAKE_CASE`, Clippy clean.
- Write idiomatic Rust: iterators over index loops, `?` over manual matching, enums over flag booleans, newtypes for units (`Ticks`, `FrameIndex`, `SampleRate`), borrow over clone.
- Descriptive names over comments. Comment only *why*, never *what*.
- No doc comments that restate the signature. Doc comments only where behavior isn't obvious from the name and types.
- Keep files small and single-purpose. Stay under 500 lines; split before reaching it.

## Testing

- Test only what can break in non-obvious ways: timestamp and timebase math, edit commands and undo/redo, ripple/roll/slip/slide logic, serialization round-trips and migrations, audio buffer and resampler sizing.
- Don't test egui layout, trivial getters/setters, derived impls, or behavior of third-party crates.
- One test per behavior; no duplicate coverage across unit and integration tests.
- Tests needing a GPU or real FFmpeg decoding are `#[ignore]` and use `tests/fixtures/`.

## Working in this repo

- Search before reading: use Grep/Glob to locate code, then read only the relevant line ranges.
- Edit in place with targeted edits. Never rewrite a whole file for a small change.
- Don't reread a file you just edited.
- All milestones (playback through proxies and hardware decode) are built. Work now is fixes, polish and performance; keep changes small and on what was asked.

## Benchmarks

Ignored tests in `crates/app/src/smoke_tests/` that print a report and need no screen. Run with `cargo test --release -p app <name> -- --ignored --nocapture`.

- `export_speed`, `playback_speed` — `EDIT_BENCH_CLIPS` (`;`-separated files), `EDIT_BENCH_SECONDS` (export), `EDIT_BENCH_DISSOLVES=1` (export), `EDIT_BENCH_EFFECTS=1` (Lumetri and blur on every clip), `EDIT_BENCH_SOFTWARE_DECODE=1`
- `vst3_plugin` — `EDIT_BENCH_VST3` (a `.vst3` bundle)
- `transcription` — `EDIT_BENCH_SPEECH_MODEL`, first file of `EDIT_BENCH_CLIPS`, optional `EDIT_BENCH_SPEECH_LANGUAGE`
- `large_project` — `EDIT_BENCH_TRACKS` (default 20 per kind), `EDIT_BENCH_TRACK_CLIPS` (default 1000)
- `frame_times_with_a_large_sequence`, `a_long_session_levels_off` (on Linux set `MALLOC_MMAP_THRESHOLD_=1048576`), `random_gestures_never_panic`; `_with_a_gpu` variants need a GPU

## Commits

Follow [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/):

- Format: `<type>(<scope>): <description>`, imperative, lowercase, no trailing period
- Types: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `build`, `ci`, `chore`, `style`
- Scope is the crate name: `feat(timeline): add ripple delete`
- Breaking changes: `!` after the scope and a `BREAKING CHANGE:` footer

## Don't

- Don't block the UI thread or the audio callback — move the work to a worker.
- Don't let FFmpeg pointers or `unsafe` escape `media/src/ffi/` — add a safe wrapper.
- Don't use floating-point time in the model — use `Ticks` with a timebase.
- Don't mutate the timeline directly — add an edit command.
- Don't write tests for trivial or third-party behavior — test the logic we own.
