//! Hosts the `plugin_fixture` gain effect, which has a combined class (component and
//! controller in one object) and a split class (separate, connected controller).
// Helpers outside `#[test]` functions fail tests by panicking, like the tests themselves.
#![allow(clippy::expect_used)]

mod support;

use plugins::{scan, ParamId, Plugin, PluginInfo};
use support::PluginFolder;

const SAMPLE_RATE: u32 = 48_000;
/// Smaller than `FRAMES` so `process` has to split every call into several blocks.
const MAX_BLOCK: usize = 64;
const FRAMES: usize = 150;
const GAIN: ParamId = ParamId(0);

fn fixture_classes(folder: &PluginFolder) -> Vec<PluginInfo> {
    scan(&[folder.root().to_owned()])
}

fn load_all(folder: &PluginFolder) -> Vec<Plugin> {
    fixture_classes(folder)
        .iter()
        .map(|info| Plugin::from_info(info, SAMPLE_RATE, MAX_BLOCK).expect("load fixture"))
        .collect()
}

fn assert_gain(plugin: &mut Plugin, gain: f32) {
    let left: Vec<f32> = (0..FRAMES).map(|i| i as f32 / FRAMES as f32).collect();
    let right: Vec<f32> = left.iter().map(|sample| -sample).collect();
    let (mut out_left, mut out_right) = (vec![0.0; FRAMES], vec![0.0; FRAMES]);
    plugin
        .process([&left, &right], [&mut out_left, &mut out_right])
        .expect("process");
    let scaled = |input: &[f32]| input.iter().map(|sample| sample * gain).collect::<Vec<_>>();
    assert_eq!(out_left, scaled(&left));
    assert_eq!(out_right, scaled(&right));
}

#[test]
fn scan_lists_only_effect_classes_and_skips_broken_bundles() {
    let folder = PluginFolder::new();
    folder.add_broken_bundle();

    let classes = fixture_classes(&folder);

    let names: Vec<&str> = classes.iter().map(|info| info.name.as_str()).collect();
    assert_eq!(names, ["Fixture Gain", "Fixture Gain Split"]);
    for info in &classes {
        assert_eq!(info.path, folder.fixture_bundle());
        assert_eq!(info.vendor, "Edit Fixtures");
        assert_eq!(info.category, "Audio Module Class");
        assert!(info.subcategories.iter().any(|sub| sub == "Fx"));
    }
}

#[test]
fn queued_parameter_reaches_processor_in_next_process_call() {
    let folder = PluginFolder::new();
    for mut plugin in load_all(&folder) {
        assert_gain(&mut plugin, 1.0);

        plugin.set_parameter(GAIN, 1.0).expect("set gain");

        assert_eq!(plugin.parameter(GAIN), 1.0);
        assert_gain(&mut plugin, 2.0);
    }
}

#[test]
fn parameters_describe_gain_with_plugin_display_text() {
    let folder = PluginFolder::new();
    for plugin in load_all(&folder) {
        let parameters = plugin.parameters();

        assert_eq!(parameters.len(), 1);
        let gain = &parameters[0];
        assert_eq!((gain.id, gain.title.as_str()), (GAIN, "Gain"));
        assert_eq!(gain.default_normalized, 0.5);
        assert_eq!(gain.step_count, 0);
        assert!(gain.flags.automatable() && !gain.flags.read_only());
        assert_eq!(plugin.display_value(GAIN, 0.5).as_deref(), Ok("1.00x"));
    }
}

#[test]
fn state_restores_unprocessed_parameter_change_in_fresh_instance() {
    let folder = PluginFolder::new();
    for info in fixture_classes(&folder) {
        let mut original = Plugin::from_info(&info, SAMPLE_RATE, MAX_BLOCK).expect("load");
        original.set_parameter(GAIN, 1.0).expect("set gain");
        let state = original.state().expect("save state");
        let mut restored = Plugin::from_info(&info, SAMPLE_RATE, MAX_BLOCK).expect("load");

        restored.set_state(&state).expect("restore state");

        assert_eq!(restored.parameter(GAIN), 1.0);
        assert_gain(&mut restored, 2.0);
    }
}

#[test]
fn module_is_shared_between_instances_and_reloads_after_last_drop() {
    let folder = PluginFolder::new();
    let info = fixture_classes(&folder).remove(0);
    let first = Plugin::from_info(&info, SAMPLE_RATE, MAX_BLOCK).expect("load first");
    let mut second = Plugin::from_info(&info, SAMPLE_RATE, MAX_BLOCK).expect("load second");

    drop(first);
    assert_gain(&mut second, 1.0);
    drop(second);

    let mut reloaded = Plugin::from_info(&info, SAMPLE_RATE, MAX_BLOCK).expect("reload");
    assert_gain(&mut reloaded, 1.0);
}
