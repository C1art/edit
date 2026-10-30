use super::*;
use crate::{
    captions_from_words, spoken_words, CaptionRules, SetTranscript, SpokenWord, Transcript, Word,
};

fn word(start: i64, end: i64, text: &str) -> Word {
    Word {
        start: Ticks(start),
        end: Ticks(end),
        text: text.into(),
    }
}

#[test]
fn words_play_where_their_clip_places_them_and_stacked_copies_count_once() {
    let (mut timeline, source) = timeline();
    let transcript = Transcript {
        language: "en".into(),
        words: vec![
            word(100, 150, "before"),
            word(180, 260, "straddles"),
            word(400, 500, "inside"),
            word(780, 900, "after"),
        ],
    };
    timeline
        .apply(SetTranscript {
            source,
            transcript: Some(Arc::new(transcript)),
        })
        .unwrap();
    timeline
        .apply(place(source, 1000, 200, 600, PlaceMode::Overwrite))
        .unwrap();
    timeline
        .apply(PlaceClip {
            video_track: None,
            audio_track: Some(1),
            ..place(source, 1000, 200, 600, PlaceMode::Overwrite)
        })
        .unwrap();
    let spoken: Vec<(i64, i64, &str)> = spoken_words(timeline.sequence())
        .iter()
        .map(|word| (word.start.0, word.end.0, word.text))
        .collect();
    assert_eq!(spoken, [(1000, 1060, "straddles"), (1200, 1300, "inside")]);
}

#[test]
fn captions_break_at_sentences_pauses_and_length_and_hold_over_short_gaps() {
    let spoken = |start: i64, end: i64, text: &'static str| SpokenWord {
        clip: ClipId(0),
        source: SourceId(0),
        start: Ticks(start),
        end: Ticks(end),
        text,
    };
    let words = [
        spoken(0, 10, "Hello"),
        spoken(12, 20, "there."),
        spoken(25, 30, "This"),
        spoken(31, 40, "line"),
        spoken(41, 50, "runs"),
        spoken(51, 60, "longer"),
        spoken(61, 70, "than"),
        spoken(71, 80, "one."),
        spoken(200, 210, "Later"),
    ];
    let rules = CaptionRules {
        line_chars: 12,
        lines: 2,
        longest: Ticks(1000),
        pause: Ticks(50),
    };
    let captions: Vec<(i64, i64, String)> = captions_from_words(&words, rules)
        .into_iter()
        .map(|caption| (caption.start.0, caption.end.0, caption.text))
        .collect();
    assert_eq!(
        captions,
        [
            (0, 25, "Hello there.".to_owned()),
            (25, 61, "This line\nruns longer".to_owned()),
            (61, 80, "than one.".to_owned()),
            (200, 210, "Later".to_owned()),
        ]
    );
}
