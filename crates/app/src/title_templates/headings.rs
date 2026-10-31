//! Lower thirds and title cards.

use timeline::{Appear, FrameSide, Pin, Rgb, Shape, TextAlign, TextStroke, Title};

use super::catalog::{graphic, rectangle, text, Built, Frame, Layer, ACCENT, INK, SOFT};

pub(super) fn accent_bar(frame: Frame) -> Built {
    let (x, y) = (frame.left(), frame.lower_third());
    let name = Title {
        bold: true,
        align: TextAlign::Left,
        shadow: true,
        ..text(frame, "Jane Doe", 64.0)
    };
    let role = Title {
        color: SOFT,
        align: TextAlign::Left,
        tracking: 40,
        shadow: true,
        ..text(frame, "Producer", 38.0)
    };
    let slide = Appear::Slide(FrameSide::Left);
    graphic([
        Layer::shape(rectangle(frame, 8.0, 124.0, Some(ACCENT)))
            .named("Bar")
            .pin(Pin::Left)
            .at(x, y + frame.px(30.0))
            .animate(
                Appear::Wipe(FrameSide::Top),
                Appear::Wipe(FrameSide::Top),
                0,
                10,
            ),
        Layer::text(name)
            .pin(Pin::Left)
            .at(x + frame.px(28.0), y)
            .animate(slide, slide, 5, 15),
        Layer::text(role)
            .pin(Pin::Left)
            .at(x + frame.px(28.0), y + frame.px(62.0))
            .animate(slide, slide, 9, 15),
    ])
}

pub(super) fn boxed_lower_third(frame: Frame) -> Built {
    let (x, y) = (frame.left(), frame.lower_third());
    let name = Title {
        bold: true,
        align: TextAlign::Left,
        background: Some(INK),
        ..text(frame, "Jane Doe", 56.0)
    };
    let role = Title {
        color: INK,
        bold: true,
        align: TextAlign::Left,
        tracking: 60,
        background: Some(ACCENT),
        ..text(frame, "PRODUCER", 32.0)
    };
    let wipe = Appear::Wipe(FrameSide::Left);
    graphic([
        Layer::text(role)
            .pin(Pin::Left)
            .at(x, y + frame.px(57.0))
            .animate(wipe, wipe, 6, 12),
        Layer::text(name)
            .pin(Pin::Left)
            .at(x, y)
            .animate(wipe, wipe, 0, 12),
    ])
}

pub(super) fn minimal_right(frame: Frame) -> Built {
    let (x, y) = (frame.right(), frame.lower_third());
    let name = Title {
        align: TextAlign::Right,
        shadow: true,
        ..text(frame, "Jane Doe", 58.0)
    };
    let role = Title {
        color: SOFT,
        align: TextAlign::Right,
        tracking: 80,
        shadow: true,
        ..text(frame, "Director of Photography", 32.0)
    };
    graphic([
        Layer::shape(rectangle(frame, 320.0, 3.0, Some(Rgb::WHITE)))
            .named("Line")
            .pin(Pin::Right)
            .at(x, y + frame.px(42.0))
            .animate(
                Appear::Wipe(FrameSide::Right),
                Appear::Wipe(FrameSide::Right),
                0,
                18,
            ),
        Layer::text(name).pin(Pin::Right).at(x, y).animate(
            Appear::Slide(FrameSide::Right),
            Appear::Slide(FrameSide::Right),
            4,
            15,
        ),
        Layer::text(role)
            .pin(Pin::Right)
            .at(x, y + frame.px(76.0))
            .animate(Appear::Fade, Appear::Fade, 10, 15),
    ])
}

pub(super) fn bold_center(frame: Frame) -> Built {
    let title = Title {
        bold: true,
        tracking: 20,
        shadow: true,
        ..text(frame, "YOUR TITLE", 150.0)
    };
    let subtitle = Title {
        color: SOFT,
        tracking: 250,
        shadow: true,
        ..text(frame, "A subtitle goes here", 44.0)
    };
    graphic([
        Layer::text(title)
            .at(0, frame.px(-30.0))
            .animate(Appear::Pop, Appear::Fade, 0, 14),
        Layer::text(subtitle)
            .at(0, frame.px(92.0))
            .animate(Appear::Fade, Appear::Fade, 10, 15),
    ])
}

pub(super) fn cinematic(frame: Frame) -> Built {
    let bar_height = 0.12 * frame.height / frame.unit;
    let bar_width = frame.width / frame.unit + 4.0;
    let bar = rectangle(frame, bar_width, bar_height, Some(Rgb::BLACK));
    // Spaced as wide as a wide frame allows; a tall one would break it over two lines.
    let (size, tracking) = if frame.is_tall() {
        (72.0, 250)
    } else {
        (96.0, 500)
    };
    let title = Title {
        color: Rgb::new(240, 236, 228),
        tracking,
        ..text(frame, "THE JOURNEY", size)
    };
    let byline = Title {
        color: SOFT,
        italic: true,
        tracking: 150,
        ..text(frame, "a film by Your Name", 36.0)
    };
    let (top, bottom) = (
        Appear::Wipe(FrameSide::Top),
        Appear::Wipe(FrameSide::Bottom),
    );
    graphic([
        Layer::shape(bar)
            .named("Top Bar")
            .at(0, frame.down(-0.44))
            .animate(top, top, 0, 20),
        Layer::shape(bar)
            .named("Bottom Bar")
            .at(0, frame.down(0.44))
            .animate(bottom, bottom, 0, 20),
        Layer::text(title).animate(Appear::Fade, Appear::Fade, 12, 30),
        Layer::text(byline)
            .at(0, frame.px(90.0))
            .animate(Appear::Fade, Appear::Fade, 24, 30),
    ])
}

pub(super) fn framed(frame: Frame) -> Built {
    let border = Shape {
        stroke: Some(TextStroke {
            color: Rgb::WHITE,
            width: frame.size(5.0),
        }),
        ..rectangle(frame, 900.0, 250.0, None)
    };
    let title = Title {
        bold: true,
        tracking: 100,
        shadow: true,
        ..text(frame, "FRAMED TITLE", 100.0)
    };
    graphic([
        Layer::shape(border).named("Frame").animate(
            Appear::Wipe(FrameSide::Left),
            Appear::Wipe(FrameSide::Right),
            0,
            16,
        ),
        Layer::text(title).animate(Appear::Fade, Appear::Fade, 8, 15),
    ])
}

pub(super) fn chapter(frame: Frame) -> Built {
    let number = Title {
        color: ACCENT,
        bold: true,
        tracking: 400,
        ..text(frame, "CHAPTER ONE", 38.0)
    };
    let heading = Title {
        shadow: true,
        ..text(frame, "The Beginning", 110.0)
    };
    let rise = Appear::Slide(FrameSide::Bottom);
    graphic([
        Layer::text(number)
            .at(0, frame.px(-88.0))
            .animate(rise, Appear::Fade, 0, 15),
        Layer::shape(rectangle(frame, 120.0, 4.0, Some(ACCENT)))
            .named("Line")
            .at(0, frame.px(-44.0))
            .animate(
                Appear::Wipe(FrameSide::Left),
                Appear::Wipe(FrameSide::Right),
                6,
                12,
            ),
        Layer::text(heading)
            .at(0, frame.px(40.0))
            .animate(rise, Appear::Fade, 10, 18),
    ])
}
