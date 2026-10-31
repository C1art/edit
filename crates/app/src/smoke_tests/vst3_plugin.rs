//! Whether a real VST3 plugin loads, processes sound and shows its editor, reported for
//! a machine whose screen nobody watches. Run it in release:
//! `EDIT_BENCH_VST3="C:\Program Files\Common Files\VST3\Some.vst3" cargo test --release -p app vst3_plugin -- --ignored --nocapture`.

use std::f32::consts::TAU;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use plugins::{EditorEvent, EditorSetup, EditorWindow, Plugin, PluginInfo};

const SAMPLE_RATE: u32 = 48_000;
const BLOCK: usize = 512;
const SECONDS: usize = 10;
/// How long the editor may take to show, and to close again.
const PATIENCE: Duration = Duration::from_secs(15);
/// How long the editor stays up, so a plugin that draws lazily gets to.
const SHOWN_FOR: Duration = Duration::from_secs(2);

#[test]
#[ignore = "loads the VST3 plugin in EDIT_BENCH_VST3; meaningful only in release"]
fn vst3_plugin() {
    let Ok(bundle) = std::env::var("EDIT_BENCH_VST3") else {
        eprintln!("Set EDIT_BENCH_VST3 to a .vst3 bundle");
        return;
    };
    let bundle = PathBuf::from(bundle);
    let mut failures = Vec::new();
    let started = Instant::now();
    let effects = match plugins::bundle_effects(&bundle) {
        Ok(effects) => effects,
        Err(error) => panic!("{}: cannot read the bundle: {error}", bundle.display()),
    };
    eprintln!(
        "{}: {} audio effect(s), read in {:.0} ms",
        bundle.display(),
        effects.len(),
        started.elapsed().as_secs_f64() * 1e3
    );
    assert!(!effects.is_empty(), "the bundle has no audio effects");
    for info in &effects {
        eprintln!(
            "\n{} by {} {} ({})",
            info.name,
            info.vendor,
            info.version,
            info.subcategories.join("|")
        );
        if let Err(error) = process(info) {
            failures.push(format!("{}: processing: {error}", info.name));
        }
        if let Err(error) = open_editor(info) {
            failures.push(format!("{}: editor: {error}", info.name));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

fn process(info: &PluginInfo) -> Result<(), String> {
    let started = Instant::now();
    let mut plugin = Plugin::from_info(info, SAMPLE_RATE, BLOCK).map_err(|e| e.to_string())?;
    eprintln!(
        "  loaded in {:.0} ms: {} parameters, latency {} samples, {}",
        started.elapsed().as_secs_f64() * 1e3,
        plugin.parameters().len(),
        plugin.latency_samples(),
        if plugin.has_editor() {
            "has an editor"
        } else {
            "no editor"
        }
    );
    let frames = SAMPLE_RATE as usize * SECONDS;
    let tone: Vec<f32> = (0..frames)
        .map(|i| 0.25 * (TAU * 440.0 * i as f32 / SAMPLE_RATE as f32).sin())
        .collect();
    let (mut left, mut right) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
    let (mut slowest, mut energy, mut finite) = (Duration::ZERO, 0.0_f64, true);
    let started = Instant::now();
    for block in tone.chunks(BLOCK) {
        let (left, right) = (&mut left[..block.len()], &mut right[..block.len()]);
        let one = Instant::now();
        plugin
            .process([block, block], [left, right])
            .map_err(|e| e.to_string())?;
        slowest = slowest.max(one.elapsed());
        finite &= left
            .iter()
            .chain(right.iter())
            .all(|sample| sample.is_finite());
        energy += left.iter().map(|&s| f64::from(s * s)).sum::<f64>();
    }
    let took = started.elapsed();
    let rms = (energy / frames as f64).sqrt();
    eprintln!(
        "  processed {SECONDS} s of a 440 Hz tone in {:.0} ms ({:.0}× real time), \
         slowest {BLOCK}-frame block {:.2} ms of {:.2} ms; output RMS {rms:.3} (input 0.177)",
        took.as_secs_f64() * 1e3,
        SECONDS as f64 / took.as_secs_f64(),
        slowest.as_secs_f64() * 1e3,
        BLOCK as f64 / f64::from(SAMPLE_RATE) * 1e3,
    );
    if !finite {
        return Err("the output has NaN or infinite samples".into());
    }
    let state = plugin.state().map_err(|e| e.to_string())?;
    eprintln!("  state saved: {} bytes", state.len());
    Ok(())
}

fn open_editor(info: &PluginInfo) -> Result<(), String> {
    if !cfg!(windows) {
        eprintln!("  editor: only shown on Windows");
        return Ok(());
    }
    let started = Instant::now();
    let window = EditorWindow::open(EditorSetup {
        path: info.path.clone(),
        class_id: info.class_id,
        title: info.name.clone(),
        state: Vec::new(),
        params: Vec::new(),
        notify: Arc::new(|| {}),
    })
    .map_err(|e| e.to_string())?;
    while !window.is_shown() {
        if let Some(error) = window.events().find_map(|event| match event {
            EditorEvent::Failed(error) => Some(error.to_string()),
            _ => None,
        }) {
            return Err(format!("did not open: {error}"));
        }
        if started.elapsed() > PATIENCE {
            return Err(format!("not shown after {PATIENCE:?}"));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    eprintln!(
        "  editor opened in {:.0} ms",
        started.elapsed().as_secs_f64() * 1e3
    );
    std::thread::sleep(SHOWN_FOR);
    let mut events: Vec<_> = window.events().collect();
    window.close();
    let closing = Instant::now();
    let finished = |events: &[EditorEvent]| {
        events
            .iter()
            .any(|event| matches!(event, EditorEvent::Closed(_) | EditorEvent::Failed(_)))
    };
    while !finished(&events) && closing.elapsed() < PATIENCE {
        std::thread::sleep(Duration::from_millis(10));
        events.extend(window.events());
    }
    for event in &events {
        match event {
            EditorEvent::Closed(state) => eprintln!(
                "  editor closed in {:.0} ms, state {}",
                closing.elapsed().as_secs_f64() * 1e3,
                state
                    .as_ref()
                    .map_or("not saved".into(), |state| format!("{} bytes", state.len()))
            ),
            EditorEvent::Failed(error) => return Err(format!("while open: {error}")),
            _ => {}
        }
    }
    if !finished(&events) {
        return Err(format!("not closed after {PATIENCE:?}"));
    }
    Ok(())
}
