#[allow(dead_code)]
#[path = "../../plugins/tests/support/mod.rs"]
mod support;

use std::time::{Duration, Instant};

use timeline::{Param, PluginParam};

use super::*;

#[test]
fn live_playback_plays_on_while_a_plugin_loads_and_through_it_once_ready() {
    let folder = support::PluginFolder::new();
    let info = plugins::scan(&[folder.root().to_owned()])
        .into_iter()
        .find(|info| info.name == "Fixture Gain")
        .unwrap();
    let doubling = PluginEffect {
        name: info.name.clone(),
        vendor: info.vendor.clone(),
        path: info.path.clone(),
        class_id: info.class_id.0,
        params: vec![PluginParam {
            id: 0,
            value: Param::from_value(1.0),
        }],
        state: PluginState::default(),
    };
    let rate = SampleRate(48_000);
    let _spares = SparePlugins::new(PluginLoading::Background);
    let mut unit = PluginUnit::new(&doubling, rate);
    let deadline = Instant::now() + Duration::from_secs(10);
    let doubled = loop {
        let mut samples = vec![0.25; 64];
        unit.process(&doubling, Ticks::ZERO, rate, &mut samples);
        assert!(
            samples
                .iter()
                .all(|&sample| sample == 0.25 || sample == 0.5),
            "{samples:?}"
        );
        if samples[0] == 0.5 || Instant::now() > deadline {
            break samples[0] == 0.5;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(doubled, "the plugin never finished loading");
}
