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
