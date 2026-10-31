use super::*;
use crate::{captions_from_cues, Caption, Captions, Generator};

fn caption(start: i64, end: i64, text: &str) -> Caption {
    Caption {
        start: Ticks(start),
        end: Ticks(end),
        text: text.to_owned(),
    }
}

#[test]
fn imported_cues_are_sorted_and_trimmed_and_overlaps_are_refused() {
    let cues = vec![
        caption(500, 900, "third"),
        caption(0, 300, "first"),
        caption(200, 500, "second"),
        caption(900, 950, "  "),
    ];
    let items = captions_from_cues(cues);
    let spans: Vec<_> = items.iter().map(|c| (c.start.0, c.end.0)).collect();
    assert_eq!(spans, [(0, 200), (200, 500), (500, 900)]);

    let (mut timeline, _) = timeline();
    let mut captions = Captions {
        items,
        ..Captions::default()
    };
    timeline
        .apply(SetCaptions {
            captions: captions.clone(),
        })
        .unwrap();
    assert_eq!(
        timeline.sequence().captions.at(Ticks(499)).unwrap().text,
        "second"
    );
    captions.items[1].end = Ticks(600);
    assert_eq!(
        timeline.apply(SetCaptions { captions }),
        Err(TimelineError::CaptionOutOfOrder(Ticks(500)))
    );
}

#[test]
fn captions_in_range_become_titles_on_a_new_top_track_and_leave_the_others() {
    let (mut timeline, _) = timeline();
    let items = vec![
        caption(0, 200, "first"),
        caption(200, 500, "second"),
        caption(600, 900, "third"),
    ];
    let captions = Captions {
        items,
        ..Captions::default()
    };
    timeline.apply(SetCaptions { captions }).unwrap();

    let range = Ticks(150)..Ticks(550);
    timeline.apply(CaptionsToTitles { range }).unwrap();

    let sequence = timeline.sequence();
    let left: Vec<_> = sequence.captions.items.iter().map(|c| &c.text).collect();
    assert_eq!(left, ["third"]);
    let titles = TrackRef::video(2);
    assert_eq!(spans(&timeline, titles), [(0, 200, 0), (200, 500, 0)]);
    let texts: Vec<_> = sequence.video[2]
        .clips
        .iter()
        .map(
            |clip| match &sequence.media_item(clip.source).unwrap().generator {
                Some(Generator::Title(title)) => title.text.as_str(),
                _ => "",
            },
        )
        .collect();
    assert_eq!(texts, ["first", "second"]);

    timeline.undo();
    assert_eq!(timeline.sequence().captions.items.len(), 3);
    assert_eq!(timeline.sequence().video.len(), 2);
}

#[test]
fn captions_ripple_with_an_insert_and_a_ripple_deleted_span() {
    let (mut timeline, source) = timeline();
    timeline
        .apply(place(source, 0, 0, 400, PlaceMode::Overwrite))
        .unwrap();
    let items = vec![
        caption(0, 100, "before"),
        caption(150, 250, "across"),
        caption(300, 400, "after"),
    ];
    timeline
        .apply(SetCaptions {
            captions: Captions {
                items,
                ..Captions::default()
            },
        })
        .unwrap();
    timeline
        .apply(place(source, 200, 0, 50, PlaceMode::Insert))
        .unwrap();
    let spans = |timeline: &Timeline| -> Vec<_> {
        let captions = &timeline.sequence().captions.items;
        captions.iter().map(|c| (c.start.0, c.end.0)).collect()
    };
    assert_eq!(
        spans(&timeline),
        [(0, 100), (150, 200), (250, 300), (350, 450)]
    );
    // The insert split the first clip at 200; deleting its 0 to 200 takes the first two.
    timeline
        .apply(RippleDelete {
            clips: vec![clip_on(&timeline, V1, 0)],
        })
        .unwrap();
    assert_eq!(spans(&timeline), [(50, 100), (150, 250)]);
    assert!(timeline.undo());
    assert!(timeline.undo());
    assert_eq!(spans(&timeline), [(0, 100), (150, 250), (300, 400)]);
}
