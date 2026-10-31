use super::*;
use crate::{Generator, Rgb, Title};

#[test]
fn generated_media_places_a_still_length_of_picture_and_edits_undo() {
    let (mut timeline, file) = timeline();
    let title = Generator::Title(Title::default());
    timeline
        .apply(AddGenerated {
            name: "Title".into(),
            generator: title.clone(),
            placed: None,
        })
        .unwrap();
    let item = timeline.sequence().media[1].clone();
    assert_eq!(
        item.default_range(Timebase::HZ_90K),
        Ticks::ZERO..Ticks(450_000)
    );
    timeline
        .apply(place(item.id, 0, 0, 450_000, PlaceMode::Overwrite))
        .unwrap();
    assert_eq!(spans(&timeline, V1), [(0, 450_000, 0)]);
    assert!(spans(&timeline, A1).is_empty());

    let matte = Generator::ColorMatte { color: Rgb::GREEN };
    let set = |source, generator: &Generator| SetGenerator {
        source,
        generator: generator.clone(),
    };
    assert_eq!(
        timeline.apply(set(file, &matte)),
        Err(TimelineError::NothingToEdit)
    );
    timeline.apply(set(item.id, &matte)).unwrap();
    timeline.undo();
    assert_eq!(timeline.sequence().media[1].generator, Some(title));
}

#[test]
fn editing_one_piece_of_a_cut_title_leaves_the_other_pieces_alone() {
    let (mut timeline, _) = timeline();
    let hello = Generator::Title(Title {
        text: "hello".into(),
        ..Title::default()
    });
    let world = Generator::Title(Title {
        text: "world".into(),
        ..Title::default()
    });
    timeline
        .apply(AddGenerated {
            name: "Title".into(),
            generator: hello.clone(),
            placed: None,
        })
        .unwrap();
    let title = timeline.sequence().media[1].id;
    timeline
        .apply(place(title, 0, 0, 400, PlaceMode::Overwrite))
        .unwrap();
    let whole = clip_on(&timeline, V1, 0);
    timeline
        .apply(AddEdits {
            cuts: vec![(whole, Ticks(200))],
        })
        .unwrap();
    let (first, second) = (clip_on(&timeline, V1, 0), clip_on(&timeline, V1, 1));
    let drawn = |timeline: &Timeline, clip| {
        let sequence = timeline.sequence();
        let (_, clip) = sequence.clip(clip).unwrap();
        sequence.media_item(clip.source).unwrap().generator.clone()
    };

    timeline
        .apply(SetClipGenerator {
            clip: second,
            generator: world.clone(),
        })
        .unwrap();
    assert_eq!(drawn(&timeline, first), Some(hello.clone()));
    assert_eq!(drawn(&timeline, second), Some(world.clone()));

    // The piece now has media of its own, so editing it again copies nothing more.
    let media = timeline.sequence().media.len();
    timeline
        .apply(SetClipGenerator {
            clip: second,
            generator: hello.clone(),
        })
        .unwrap();
    assert_eq!(timeline.sequence().media.len(), media);

    timeline.undo();
    timeline.undo();
    assert_eq!(drawn(&timeline, second), Some(hello));
    assert_eq!(timeline.sequence().media.len(), media - 1);
}
