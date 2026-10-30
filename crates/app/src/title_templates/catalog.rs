//! The title templates that come with the editor, laid out for whatever frame they land
//! in: sizes follow the frame's shorter side and places its edges, so a vertical
//! sequence gets them as fitting as a wide one.

use timeline::{
    Appear, Generator, Graphic, GraphicContent, GraphicLayer, Pin, Rgb, Shape, ShapeKind, Title,
};

use super::headings::{
    accent_bar, bold_center, boxed_lower_third, chapter, cinematic, framed, minimal_right,
};
use super::overlays::{callout, credits, end_card, news_crawl, quote, social_handle};

pub(super) const ACCENT: Rgb = Rgb::new(255, 186, 48);
pub(super) const INK: Rgb = Rgb::new(18, 18, 22);
pub(super) const SOFT: Rgb = Rgb::new(214, 214, 220);
pub(super) const RED: Rgb = Rgb::new(230, 33, 23);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Category {
    LowerThirds,
    Titles,
    Callouts,
    Social,
    Moving,
}

impl Category {
    pub(super) const ALL: [Self; 5] = [
        Self::LowerThirds,
        Self::Titles,
        Self::Callouts,
        Self::Social,
        Self::Moving,
    ];

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::LowerThirds => "Lower Thirds",
            Self::Titles => "Titles",
            Self::Callouts => "Callouts and Quotes",
            Self::Social => "Social",
            Self::Moving => "Rolls and Crawls",
        }
    }
}

pub(super) struct Builtin {
    pub(super) name: &'static str,
    pub(super) category: Category,
    build: fn(Frame) -> Built,
}

/// A template made for one frame: what to generate, how many seconds of it to place
/// when not the usual still length, and where to move the clip when not centered.
pub(super) struct Built {
    pub(super) generator: Generator,
    pub(super) seconds: Option<i64>,
    pub(super) position: Option<[i32; 2]>,
}

impl Builtin {
    pub(super) fn build(&self, width: u32, height: u32) -> Built {
        (self.build)(Frame::new(width, height))
    }
}

pub(super) const BUILTINS: [Builtin; 13] = [
    Builtin {
        name: "Accent Bar",
        category: Category::LowerThirds,
        build: accent_bar,
    },
    Builtin {
        name: "Boxed",
        category: Category::LowerThirds,
        build: boxed_lower_third,
    },
    Builtin {
        name: "Minimal Right",
        category: Category::LowerThirds,
        build: minimal_right,
    },
    Builtin {
        name: "Bold Center",
        category: Category::Titles,
        build: bold_center,
    },
    Builtin {
        name: "Cinematic",
        category: Category::Titles,
        build: cinematic,
    },
    Builtin {
        name: "Framed",
        category: Category::Titles,
        build: framed,
    },
    Builtin {
        name: "Chapter Heading",
        category: Category::Titles,
        build: chapter,
    },
    Builtin {
        name: "Callout",
        category: Category::Callouts,
        build: callout,
    },
    Builtin {
        name: "Quote",
        category: Category::Callouts,
        build: quote,
    },
    Builtin {
        name: "Social Handle",
        category: Category::Social,
        build: social_handle,
    },
    Builtin {
        name: "End Card",
        category: Category::Social,
        build: end_card,
    },
    Builtin {
        name: "Credits Roll",
        category: Category::Moving,
        build: credits,
    },
    Builtin {
        name: "News Crawl",
        category: Category::Moving,
        build: news_crawl,
    },
];

#[derive(Clone, Copy)]
pub(super) struct Frame {
    pub(super) width: f32,
    pub(super) height: f32,
    /// Pixels per unit of a 1080-line design.
    pub(super) unit: f32,
}

impl Frame {
    fn new(width: u32, height: u32) -> Self {
        let (width, height) = (width.max(1) as f32, height.max(1) as f32);
        Self {
            width,
            height,
            unit: width.min(height) / 1080.0,
        }
    }

    /// `units` of a 1080-line design in this frame's pixels.
    pub(super) fn px(self, units: f32) -> i32 {
        (units * self.unit).round() as i32
    }

    pub(super) fn size(self, units: f32) -> u32 {
        self.px(units).max(1).unsigned_abs()
    }

    /// From the center to `share` of the width, left negative.
    pub(super) fn across(self, share: f32) -> i32 {
        (share * self.width).round() as i32
    }

    pub(super) fn down(self, share: f32) -> i32 {
        (share * self.height).round() as i32
    }

    /// Taller than wide, as phone video is.
    pub(super) fn is_tall(self) -> bool {
        self.height > self.width
    }

    /// The left edge of the title-safe area.
    pub(super) fn left(self) -> i32 {
        self.across(-0.4)
    }

    pub(super) fn right(self) -> i32 {
        self.across(0.4)
    }

    /// Where a lower third's first line sits.
    pub(super) fn lower_third(self) -> i32 {
        self.down(0.27)
    }
}

pub(super) fn text(frame: Frame, text: &str, size: f32) -> Title {
    Title {
        text: text.to_owned(),
        size: frame.size(size),
        shadow: false,
        ..Title::default()
    }
}

pub(super) fn rectangle(frame: Frame, width: f32, height: f32, fill: Option<Rgb>) -> Shape {
    Shape {
        width: frame.size(width),
        height: frame.size(height),
        fill,
        ..Shape::new(ShapeKind::Rectangle)
    }
}

/// A layer built up from its content.
pub(super) struct Layer(GraphicLayer);

impl Layer {
    pub(super) fn text(title: Title) -> Self {
        Self(GraphicLayer::new(GraphicContent::Text(title)))
    }

    pub(super) fn shape(shape: Shape) -> Self {
        Self(GraphicLayer::new(GraphicContent::Shape(shape)))
    }

    pub(super) fn named(mut self, name: &str) -> Self {
        name.clone_into(&mut self.0.name);
        self
    }

    pub(super) fn at(mut self, x: i32, y: i32) -> Self {
        self.0.offset = [x, y];
        self
    }

    pub(super) fn pin(mut self, pin: Pin) -> Self {
        self.0.pin = pin;
        self
    }

    pub(super) fn turned(mut self, degrees: i32) -> Self {
        self.0.rotation = degrees;
        self
    }

    pub(super) fn faded(mut self, opacity: u32) -> Self {
        self.0.opacity = opacity;
        self
    }

    /// Comes on with `enter` and goes off with `exit`, `delay` frames in from each end.
    pub(super) fn animate(mut self, enter: Appear, exit: Appear, delay: u32, length: u32) -> Self {
        let animation = &mut self.0.animation;
        (animation.enter, animation.exit) = (enter, exit);
        (animation.delay, animation.length) = (delay, length);
        self
    }
}

/// Layers listed bottom first, as they're designed, kept topmost first.
pub(super) fn graphic(layers: impl IntoIterator<Item = Layer>) -> Built {
    let mut layers: Vec<_> = layers.into_iter().map(|layer| layer.0).collect();
    layers.reverse();
    Built {
        generator: Generator::Graphic(Graphic { layers }),
        seconds: None,
        position: None,
    }
}
impl Built {
    pub(super) fn lasting(mut self, seconds: i64) -> Self {
        self.seconds = Some(seconds);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_builds_inside_wide_and_tall_frames() {
        for (width, height) in [(1920, 1080), (1080, 1920), (3840, 2160)] {
            for builtin in &BUILTINS {
                let built = builtin.build(width, height);
                let Generator::Graphic(graphic) = built.generator else {
                    continue;
                };
                for layer in graphic.layers {
                    let [x, y] = layer.offset;
                    assert!(
                        x.unsigned_abs() < width / 2 && y.unsigned_abs() < height / 2,
                        "{} {} at {x},{y} in {width}x{height}",
                        builtin.name,
                        layer.name
                    );
                }
            }
        }
    }
}
