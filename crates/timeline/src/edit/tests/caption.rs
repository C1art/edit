use super::*;
use crate::{captions_from_cues, Caption, Captions};

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
