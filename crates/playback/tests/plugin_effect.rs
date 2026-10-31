// Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[allow(dead_code)]
#[path = "../../plugins/tests/support/mod.rs"]
mod support;

use std::sync::Arc;

use audio::SampleRate;
use playback::MonoMix;
use support::PluginFolder;
use timeline::{
    AddGenerated, AudioEffect, Generator, Param, PlaceClip, PlaceMode, PluginEffect, PluginParam,
    PluginState, Sequence, Ticks, Timebase, Timeline, VideoFormat,
};

const TB: Timebase = Timebase::HZ_90K;
const RATE: SampleRate = SampleRate(48_000);

/// A second of tone on A1, with `master` as the master track's effects.
fn tone(master: Vec<AudioEffect>) -> Arc<Sequence> {
    let mut timeline = Timeline::new(Sequence::with_default_tracks(TB, VideoFormat::default()));
    let generator = Generator::BarsAndTone;
    let name = generator.kind_name().into();
    timeline
        .apply(AddGenerated {
            name,
            generator,
            placed: None,
        })
        .unwrap();
    let source = timeline.sequence().media[0].id;
    timeline
        .apply(PlaceClip {
            source,
            source_in: Ticks::ZERO,
            duration: Ticks(90_000),
            at: Ticks::ZERO,
            video_track: None,
            audio_track: Some(0),
            mode: PlaceMode::Overwrite,
            fill: None,
        })
        .unwrap();
    let mut sequence = timeline.sequence().clone();
    sequence.master_effects = master;
    Arc::new(sequence)
}

fn peak(sequence: Arc<Sequence>) -> f32 {
    let range = Ticks::ZERO..sequence.duration();
    let mut mix = MonoMix::new((sequence, range), RATE);
    let (mut samples, mut warnings) = (Vec::new(), Vec::new());
    mix.read(mix.len(), &mut samples, &mut warnings);
    assert!(warnings.is_empty(), "{warnings:?}");
    samples
        .iter()
        .fold(0.0, |peak, sample| peak.max(sample.abs()))
}

#[test]
fn a_plugin_on_the_master_track_plays_with_its_saved_state_and_changed_parameters() {
    let folder = PluginFolder::new();
    let info = plugins::scan(&[folder.root().to_owned()])
        .into_iter()
        .find(|info| info.name == "Fixture Gain")
        .unwrap();
    let mut saved = plugins::Plugin::from_info(&info, RATE.0, 512).unwrap();
    saved.set_parameter(plugins::ParamId(0), 1.0).unwrap();
    let doubling = PluginState::new(saved.state().unwrap());
    let gain = |value: Option<f32>, state: &PluginState| {
        AudioEffect::Plugin(Box::new(PluginEffect {
            name: info.name.clone(),
            vendor: info.vendor.clone(),
            path: info.path.clone(),
            class_id: info.class_id.0,
            params: value
                .map(|value| PluginParam {
                    id: 0,
                    value: Param::from_value(value),
                })
                .into_iter()
                .collect(),
            state: state.clone(),
        }))
    };
    let plain = PluginState::default();
    let dry = peak(tone(Vec::new()));
    let unity = peak(tone(vec![gain(None, &plain)]));
    let doubled = peak(tone(vec![gain(Some(1.0), &plain)]));
    let restored = peak(tone(vec![gain(None, &doubling)]));
    let overridden = peak(tone(vec![gain(Some(0.5), &doubling)]));
    let after = peak(tone(vec![gain(None, &plain)]));
    assert!(dry > 0.01, "{dry}");
    assert!((unity - dry).abs() < 1e-4, "{dry} then {unity}");
    assert!((doubled - 2.0 * dry).abs() < 1e-4, "{dry} then {doubled}");
    assert!((restored - 2.0 * dry).abs() < 1e-4, "{dry} then {restored}");
    assert!((overridden - dry).abs() < 1e-4, "{dry} then {overridden}");
    assert!((after - dry).abs() < 1e-4, "{dry} then reused at {after}");
}
