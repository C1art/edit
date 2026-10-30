use std::sync::Arc;

use super::*;
use crate::{AddGenerated, Generator, Title};

/// A title on V1 from 0 to 100, and a 1000-tick file of its pictures.
fn rendered_title() -> (Timeline, SourceId, SourceId) {
    let (mut timeline, _) = timeline();
    timeline
        .apply(AddGenerated {
            name: "Title".into(),
            generator: Generator::Title(Title::default()),
            placed: None,
        })
        .unwrap();
    let title = timeline.sequence().media[1].id;
    timeline
        .apply(PlaceClip {
            audio_track: None,
            ..place(title, 0, 40, 100, PlaceMode::Overwrite)
        })
        .unwrap();
    timeline
        .apply(ImportMedia {
            name: "Title rendered".into(),
            path: "title.mov".into(),
            duration: Ticks(1000),
            video: Some(timeline.sequence().format),
            has_audio: false,
            still: false,
            interpreted_rate: None,
            placed: None,
            frame_fit: None,
        })
        .unwrap();
    let rendered = timeline.sequence().media[2].id;
    (timeline, title, rendered)
}

#[test]
fn render_and_replace_swaps_the_source_at_the_same_times_and_restores() {
    let (mut timeline, title, rendered) = rendered_title();
    let clip = clip_on(&timeline, V1, 0);
    let replace = RenderAndReplace {
        clips: vec![clip],
        original: title,
        rendered,
    };
    timeline.apply(replace.clone()).unwrap();
    let shown = |timeline: &Timeline| {
        let (_, clip) = timeline.sequence().clip(clip).unwrap();
        (clip.source, clip.source_in, clip.unrendered)
    };
    assert_eq!(shown(&timeline), (rendered, Ticks(40), Some(title)));
    assert_eq!(
        timeline.apply(RemoveMedia { source: title }),
        Err(TimelineError::MediaInUse(title))
    );
    // Already showing the file, so there is nothing left to replace.
    assert_eq!(timeline.apply(replace), Err(TimelineError::NothingToEdit));

    timeline
        .apply(RestoreUnrendered { clips: vec![clip] })
        .unwrap();
    assert_eq!(shown(&timeline), (title, Ticks(40), None));
    assert_eq!(
        timeline.apply(RestoreUnrendered { clips: vec![clip] }),
        Err(TimelineError::NothingToEdit)
    );
    timeline.undo();
    timeline.undo();
    assert_eq!(shown(&timeline), (title, Ticks(40), None));
}

#[test]
fn graphics_render_alone_but_adjustment_layers_do_not() {
    let (mut timeline, title, rendered) = rendered_title();
    let view = timeline.sequence().render_view(title).unwrap();
    let clips: Vec<_> = view
        .tracks()
        .flat_map(|(_, track)| track.clips.iter().map(Arc::as_ref))
        .collect();
    assert_eq!(clips.len(), 1);
    assert_eq!((clips[0].source, clips[0].start), (title, Ticks::ZERO));
    assert_eq!(
        clips[0].source_out(),
        view.media_item(title).unwrap().duration
    );
    assert!(timeline.sequence().render_view(rendered).is_none());
    timeline
        .apply(AddGenerated {
            name: "Adjustment".into(),
            generator: Generator::AdjustmentLayer,
            placed: None,
        })
        .unwrap();
    let adjustment = timeline.sequence().media[3].id;
    assert!(timeline.sequence().render_view(adjustment).is_none());
}

#[test]
fn a_render_too_short_for_the_clip_is_refused() {
    let (mut timeline, title, _) = rendered_title();
    timeline
        .apply(ImportMedia {
            name: "Short".into(),
            path: "short.mov".into(),
            duration: Ticks(120),
            video: Some(timeline.sequence().format),
            has_audio: false,
            still: false,
            interpreted_rate: None,
            placed: None,
            frame_fit: None,
        })
        .unwrap();
    let short = timeline.sequence().media[3].id;
    let replace = RenderAndReplace {
        clips: vec![clip_on(&timeline, V1, 0)],
        original: title,
        rendered: short,
    };
    assert_eq!(timeline.apply(replace), Err(TimelineError::NotEnoughSource));
}
