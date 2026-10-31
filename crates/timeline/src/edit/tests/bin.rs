use super::*;
use crate::{BinId, BinSearch, SearchField};

fn add_bin(timeline: &mut Timeline, parent: Option<BinId>) -> BinId {
    timeline
        .apply(AddBin {
            name: "Bin".into(),
            parent,
            search: None,
        })
        .unwrap();
    timeline.sequence().bins.last().unwrap().id
}

#[test]
fn a_bin_cannot_move_inside_itself_or_its_descendants() {
    let (mut timeline, _) = timeline();
    let outer = add_bin(&mut timeline, None);
    let inner = add_bin(&mut timeline, Some(outer));
    let to = |to| MoveToBin {
        media: Vec::new(),
        bins: vec![outer],
        to: Some(to),
    };
    assert_eq!(
        timeline.apply(to(inner)),
        Err(TimelineError::BinCycle(outer))
    );
    assert_eq!(
        timeline.apply(to(outer)),
        Err(TimelineError::BinCycle(outer))
    );
    timeline
        .apply(MoveToBin {
            media: Vec::new(),
            bins: vec![inner],
            to: None,
        })
        .unwrap();
    timeline.apply(to(inner)).unwrap();
    assert_eq!(timeline.sequence().bin(outer).unwrap().parent, Some(inner));
}

#[test]
fn deleting_a_bin_hands_its_contents_to_the_bin_around_it() {
    let (mut timeline, source) = timeline();
    let outer = add_bin(&mut timeline, None);
    let middle = add_bin(&mut timeline, Some(outer));
    let inner = add_bin(&mut timeline, Some(middle));
    timeline
        .apply(MoveToBin {
            media: vec![source],
            bins: Vec::new(),
            to: Some(middle),
        })
        .unwrap();
    timeline.apply(RemoveBin { bin: middle }).unwrap();
    let sequence = timeline.sequence();
    assert_eq!(sequence.media_item(source).unwrap().bin, Some(outer));
    assert_eq!(sequence.bin(inner).unwrap().parent, Some(outer));
    assert!(sequence.bin(middle).is_none());
}

#[test]
fn a_search_bin_shows_what_matches_and_takes_nothing_in() {
    let (mut timeline, source) = timeline();
    let search = |text: &str, field| BinSearch {
        text: text.into(),
        field,
    };
    timeline
        .apply(AddBin {
            name: "Found".into(),
            parent: None,
            search: Some(search("A.MP", SearchField::FilePath)),
        })
        .unwrap();
    let found = timeline.sequence().bins.last().unwrap().id;
    let shown = |timeline: &Timeline| -> Vec<SourceId> {
        let sequence = timeline.sequence();
        sequence
            .bin_items(Some(found))
            .map(|item| item.id)
            .collect()
    };
    assert_eq!(shown(&timeline), vec![source]);
    timeline
        .apply(SetBinSearch {
            bin: found,
            search: search("A.MP", SearchField::Name),
        })
        .unwrap();
    assert!(shown(&timeline).is_empty());
    let move_in = MoveToBin {
        media: vec![source],
        bins: Vec::new(),
        to: Some(found),
    };
    assert_eq!(
        timeline.apply(move_in),
        Err(TimelineError::SearchBin(found))
    );
}
