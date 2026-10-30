// Helpers outside `#[test]` fns are not covered by `allow-unwrap-in-tests`.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use media::MediaReader;
use playback::{LoudnessEvent, LoudnessJob};
use timeline::{
    preview, AddSubmix, Gain, ImportMedia, PlaceClip, PlaceMode, Sequence, SetSubmix,
    SetTrackOutput, Ticks, Timebase, Timeline, TrackRef, VideoFormat,
};

const TB: Timebase = Timebase::HZ_90K;

/// The fixture's sound on A1, sent to a submix.
fn routed() -> Timeline {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/testsrc_2s_25fps_44k.mp4");
    let duration = MediaReader::open(&path, TB)
        .unwrap()
        .info()
        .duration
        .unwrap();
    let mut timeline = Timeline::new(Sequence::with_default_tracks(TB, VideoFormat::default()));
    timeline
        .apply(ImportMedia {
            name: "testsrc".into(),
            path,
            duration,
            video: None,
            has_audio: true,
            still: false,
            interpreted_rate: None,
            placed: None,
            frame_fit: None,
        })
        .unwrap();
    let source = timeline.sequence().media[0].id;
    timeline
        .apply(PlaceClip {
            source,
            source_in: Ticks::ZERO,
            duration,
            at: Ticks::ZERO,
            video_track: None,
            audio_track: Some(0),
            mode: PlaceMode::Overwrite,
            fill: None,
        })
        .unwrap();
    timeline.apply(AddSubmix).unwrap();
    let output = Some(timeline.sequence().submixes[0].id);
    let track = TrackRef::audio(0);
    timeline.apply(SetTrackOutput { track, output }).unwrap();
    timeline
}

fn with_submix(timeline: &Timeline, change: impl FnOnce(&mut timeline::Submix)) -> Sequence {
    let mut submix = timeline.sequence().submixes[0].clone();
    change(&mut submix);
    preview(timeline.sequence(), &SetSubmix { submix }).unwrap()
}

#[test]
#[ignore = "decodes a real file with FFmpeg"]
fn a_submix_fader_and_mute_act_on_the_tracks_sent_to_it() {
    let timeline = routed();
    let range = Ticks::ZERO..timeline.sequence().duration();
    let sequences = [
        timeline.sequence().clone(),
        with_submix(&timeline, |submix| submix.volume = Gain::from_db(-6.0)),
        with_submix(&timeline, |submix| submix.muted = true),
    ];
    let pieces = sequences
        .into_iter()
        .map(|sequence| (Arc::new(sequence), range.clone()))
        .collect();
    let job = LoudnessJob::start(pieces).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let measured = loop {
        assert!(Instant::now() < deadline, "timed out");
        let events = job.poll();
        if let Some(LoudnessEvent::Measured(measured)) = events.into_iter().last() {
            break measured;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let unity = measured[0].integrated.unwrap();
    let lowered = measured[1].integrated.unwrap();
    assert!(
        (unity - lowered - 6.0).abs() < 0.1,
        "{unity} then {lowered}"
    );
    assert_eq!(measured[2].integrated, None);
}
