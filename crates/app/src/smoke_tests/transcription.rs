//! How long speech to text takes on a real clip, and what it heard. Run it in release
//! with a Whisper model file and the clip:
//! `EDIT_BENCH_SPEECH_MODEL="ggml-base.en.bin" EDIT_BENCH_CLIPS="talk.mp4" cargo test --release -p app transcription -- --ignored --nocapture`.
//! `EDIT_BENCH_SPEECH_LANGUAGE` sets the language, as "en"; otherwise the model tells.

use std::time::{Duration, Instant};

use speech::{TranscribeEvent, TranscribeJob};
use timeline::Timebase;

use super::{bench_clips, editor};

const WORDS_PER_LINE: usize = 12;
const LINES: usize = 8;
const PATIENCE: Duration = Duration::from_secs(60 * 60);

#[test]
#[ignore = "transcribes the first file in EDIT_BENCH_CLIPS; meaningful only in release"]
fn transcription() {
    let Ok(model) = std::env::var("EDIT_BENCH_SPEECH_MODEL") else {
        eprintln!("Set EDIT_BENCH_SPEECH_MODEL to a Whisper ggml model file");
        return;
    };
    let mut editor = editor(None);
    let Some(media) = bench_clips::import(&mut editor) else {
        return;
    };
    let Some(&(source, duration)) = media.first() else {
        return;
    };
    let sequence = editor.timeline.sequence();
    let timebase = sequence.timebase;
    let seconds = |ticks: timeline::Ticks| {
        Timebase::new(1, 1000).map_or(0.0, |ms| ticks.rescale(timebase, ms).0 as f64 / 1e3)
    };
    let piece = playback::media_alone(sequence, source).expect("the clip has sound");
    let language = std::env::var("EDIT_BENCH_SPEECH_LANGUAGE").ok();
    let started = Instant::now();
    let job = TranscribeJob::start(piece, model.into(), language).expect("a transcription job");
    let transcript = loop {
        let mut done = None;
        for event in job.poll() {
            match event {
                TranscribeEvent::Done(transcript) => done = Some(transcript),
                TranscribeEvent::Failed(error) => panic!("transcription failed: {error}"),
                TranscribeEvent::Warning(warning) => eprintln!("warning: {warning}"),
                TranscribeEvent::Progress(_) => {}
            }
        }
        if let Some(transcript) = done {
            break transcript;
        }
        assert!(started.elapsed() < PATIENCE, "still transcribing");
        std::thread::sleep(Duration::from_millis(20));
    };
    let took = started.elapsed().as_secs_f64();
    let clip = seconds(duration);
    eprintln!(
        "Transcribed {clip:.1} s in {took:.1} s ({:.1}× real time): {} words, language \"{}\"",
        clip / took.max(1e-9),
        transcript.words.len(),
        transcript.language
    );
    for line in transcript.words.chunks(WORDS_PER_LINE).take(LINES) {
        let text: Vec<_> = line.iter().map(|word| word.text.trim()).collect();
        eprintln!("  [{:7.2} s] {}", seconds(line[0].start), text.join(" "));
    }
}
