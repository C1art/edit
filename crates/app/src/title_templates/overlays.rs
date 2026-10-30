//! Callouts, quotes, social graphics, and titles that roll or crawl.

use timeline::{
    Appear, FrameSide, Generator, Pin, Rgb, Scroll, Shape, ShapeKind, TextAlign, TextStroke, Title,
    TitleMotion,
};

use super::catalog::{graphic, rectangle, text, Built, Frame, Layer, ACCENT, INK, RED, SOFT};

pub(super) fn callout(frame: Frame) -> Built {
    let ring = Shape {
        stroke: Some(TextStroke {
            color: ACCENT,
            width: frame.size(8.0),
        }),
        ..Shape {
            width: frame.size(240.0),
            height: frame.size(240.0),
            fill: None,
            ..Shape::new(ShapeKind::Ellipse)
        }
    };
    let label = Title {
        bold: true,
        align: TextAlign::Left,
        background: Some(INK),
        ..text(frame, "Look here", 44.0)
    };
    // The line runs from the ring's edge up to the label's left end.
    let wipe = Appear::Wipe(FrameSide::Left);
    graphic([
        Layer::shape(ring)
            .named("Ring")
            .animate(Appear::Pop, Appear::Fade, 0, 12),
        Layer::shape(rectangle(frame, 100.0, 4.0, Some(ACCENT)))
            .named("Pointer")
            .at(frame.px(128.0), frame.px(-108.0))
            .turned(-28)
            .animate(wipe, Appear::Fade, 8, 8),
        Layer::text(label)
            .pin(Pin::Left)
            .at(frame.px(172.0), frame.px(-132.0))
            .animate(wipe, Appear::Fade, 12, 12),
    ])
}

pub(super) fn quote(frame: Frame) -> Built {
    let mark = Title {
        color: ACCENT,
        bold: true,
        ..text(frame, "\u{201C}", 220.0)
    };
    let words = Title {
        italic: true,
        shadow: true,
        ..text(frame, "Simplicity is the ultimate\nsophistication.", 60.0)
    };
    let source = Title {
        color: SOFT,
        tracking: 60,
        ..text(frame, "\u{2014} Leonardo da Vinci", 38.0)
    };
    graphic([
        Layer::text(mark)
            .named("Quote Mark")
            .at(0, frame.px(-150.0))
            .animate(Appear::Pop, Appear::Fade, 0, 14),
        Layer::text(words).animate(Appear::Fade, Appear::Fade, 6, 20),
        Layer::text(source).at(0, frame.px(130.0)).animate(
            Appear::Slide(FrameSide::Bottom),
            Appear::Fade,
            16,
            15,
        ),
    ])
}

pub(super) fn social_handle(frame: Frame) -> Built {
    let (x, y) = (frame.left(), frame.down(0.36));
    let pill = Shape {
        corner_radius: frame.size(48.0),
        ..rectangle(frame, 560.0, 96.0, Some(INK))
    };
    let dot = Shape {
        width: frame.size(60.0),
        height: frame.size(60.0),
        fill: Some(ACCENT),
        ..Shape::new(ShapeKind::Ellipse)
    };
    let handle = Title {
        bold: true,
        align: TextAlign::Left,
        ..text(frame, "@yourhandle", 46.0)
    };
    graphic([
        Layer::shape(pill)
            .named("Pill")
            .pin(Pin::Left)
            .at(x, y)
            .faded(88)
            .animate(
                Appear::Wipe(FrameSide::Left),
                Appear::Wipe(FrameSide::Left),
                0,
                12,
            ),
        Layer::shape(dot)
            .named("Icon")
            .at(x + frame.px(56.0), y)
            .animate(Appear::Pop, Appear::Fade, 6, 12),
        Layer::text(handle)
            .pin(Pin::Left)
            .at(x + frame.px(104.0), y)
            .animate(Appear::Slide(FrameSide::Left), Appear::Fade, 8, 14),
    ])
}

pub(super) fn end_card(frame: Frame) -> Built {
    // Side by side in a wide frame; stacked and larger in a tall one.
    let box_width = if frame.is_tall() { 0.7 } else { 0.3 } * frame.width / frame.unit;
    let placeholder = Shape {
        corner_radius: frame.size(12.0),
        stroke: Some(TextStroke {
            color: Rgb::WHITE,
            width: frame.size(4.0),
        }),
        ..rectangle(frame, box_width, box_width * 9.0 / 16.0, Some(Rgb::WHITE))
    };
    let heading = Title {
        bold: true,
        shadow: true,
        ..text(frame, "Thanks for watching!", 80.0)
    };
    let label = Title {
        color: SOFT,
        ..text(frame, "Watch next", 40.0)
    };
    let button = Shape {
        corner_radius: frame.size(48.0),
        ..rectangle(frame, 380.0, 96.0, Some(RED))
    };
    let subscribe = Title {
        bold: true,
        tracking: 80,
        ..text(frame, "SUBSCRIBE", 44.0)
    };
    let (left, right) = if frame.is_tall() {
        ([0, frame.down(-0.12)], [0, frame.down(0.12)])
    } else {
        let middle = frame.down(0.02);
        ([frame.across(-0.22), middle], [frame.across(0.22), middle])
    };
    let button_y = frame.down(if frame.is_tall() { 0.36 } else { 0.34 });
    graphic([
        Layer::text(heading).at(0, frame.down(-0.34)).animate(
            Appear::Slide(FrameSide::Top),
            Appear::Fade,
            0,
            15,
        ),
        Layer::shape(placeholder)
            .named("Video Left")
            .at(left[0], left[1])
            .faded(14)
            .animate(Appear::Pop, Appear::Fade, 8, 14),
        Layer::shape(placeholder)
            .named("Video Right")
            .at(right[0], right[1])
            .faded(14)
            .animate(Appear::Pop, Appear::Fade, 12, 14),
        Layer::text(label.clone())
            .at(left[0], left[1])
            .animate(Appear::Fade, Appear::Fade, 16, 12),
        Layer::text(label)
            .at(right[0], right[1])
            .animate(Appear::Fade, Appear::Fade, 20, 12),
        Layer::shape(button)
            .named("Button")
            .at(0, button_y)
            .animate(Appear::Pop, Appear::Fade, 22, 14),
        Layer::text(subscribe)
            .at(0, button_y)
            .animate(Appear::Fade, Appear::Fade, 26, 10),
    ])
    .lasting(10)
}

pub(super) fn credits(frame: Frame) -> Built {
    let roll = Title {
        text: [
            "THE END",
            "",
            "",
            "Directed by",
            "Your Name",
            "",
            "Produced by",
            "Your Name",
            "",
            "Edited by",
            "Your Name",
            "",
            "Music",
            "Artist Name",
            "",
            "",
            "Thank you for watching",
        ]
        .join("\n"),
        leading: frame.px(8.0),
        motion: TitleMotion::Roll(Scroll::default()),
        ..text(frame, "", 48.0)
    };
    Built {
        generator: Generator::Title(roll),
        seconds: Some(20),
        position: None,
    }
}

pub(super) fn news_crawl(frame: Frame) -> Built {
    let crawl = Title {
        bold: true,
        background: Some(Rgb::new(180, 20, 30)),
        motion: TitleMotion::CrawlLeft(Scroll::default()),
        ..text(
            frame,
            "BREAKING  \u{2022}  Your headline crawls across here  \u{2022}  \
             Add more stories between the bullets  \u{2022}",
            44.0,
        )
    };
    Built {
        generator: Generator::Title(crawl),
        seconds: Some(15),
        position: Some([0, frame.down(0.4)]),
    }
}
