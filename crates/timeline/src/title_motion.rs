use serde::{Deserialize, Serialize};

/// How a title travels across the screen over its clip, as Premiere's rolls and crawls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TitleMotion {
    #[default]
    Still,
    /// Up the screen, as end credits.
    Roll(Scroll),
    CrawlLeft(Scroll),
    CrawlRight(Scroll),
}

impl TitleMotion {
    pub fn name(self) -> &'static str {
        match self {
            Self::Still => "Still",
            Self::Roll(_) => "Roll",
            Self::CrawlLeft(_) => "Crawl Left",
            Self::CrawlRight(_) => "Crawl Right",
        }
    }

    pub fn scroll(self) -> Option<Scroll> {
        match self {
            Self::Still => None,
            Self::Roll(scroll) | Self::CrawlLeft(scroll) | Self::CrawlRight(scroll) => Some(scroll),
        }
    }

    /// The same motion with `scroll`'s timing, for switching between kinds.
    pub fn with_scroll(self, scroll: Scroll) -> Self {
        match self {
            Self::Still => Self::Still,
            Self::Roll(_) => Self::Roll(scroll),
            Self::CrawlLeft(_) => Self::CrawlLeft(scroll),
            Self::CrawlRight(_) => Self::CrawlRight(scroll),
        }
    }

    /// Crawls lay their text out on one line, however long.
    pub fn is_crawl(self) -> bool {
        matches!(self, Self::CrawlLeft(_) | Self::CrawlRight(_))
    }

    /// Where the leading edge of a title `block` pixels long sits along its travel, from
    /// the top or left of a screen `view` pixels long, at `frame` of a clip `frames` long.
    pub fn position(self, block: f32, view: f32, frame: i64, frames: i64) -> f32 {
        let Some(scroll) = self.scroll() else {
            return (view - block) / 2.0;
        };
        let position = scroll.position(block, view, frame, frames);
        match self {
            // Mirrored, so it enters from the left and leaves to the right.
            Self::CrawlRight(_) => view - block - position,
            _ => position,
        }
    }
}

/// When and from where to where a roll or crawl travels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Scroll {
    /// Enters from beyond the screen's edge, rather than starting where it lies still.
    pub start_off_screen: bool,
    /// Leaves the screen entirely, rather than stopping with its far end in view.
    pub end_off_screen: bool,
    /// Frames held at the start before moving.
    pub preroll: u32,
    /// Frames spent speeding up from a stop.
    pub ease_in: u32,
    /// Frames spent slowing to a stop.
    pub ease_out: u32,
    /// Frames held at the end after stopping.
    pub postroll: u32,
}

impl Default for Scroll {
    fn default() -> Self {
        Self {
            start_off_screen: true,
            end_off_screen: true,
            preroll: 0,
            ease_in: 0,
            ease_out: 0,
            postroll: 0,
        }
    }
}

impl Scroll {
    /// The leading edge's distance from the screen's, moving towards smaller values.
    fn position(self, block: f32, view: f32, frame: i64, frames: i64) -> f32 {
        let still = (view - block) / 2.0;
        // Longer than the screen, it starts with its head at the top and ends with its
        // tail at the bottom.
        let start = if self.start_off_screen {
            view
        } else {
            still.max(0.0)
        };
        let end = if self.end_off_screen {
            -block
        } else {
            still.min(view - block)
        };
        start + (end - start) * self.progress(frame, frames)
    }

    /// 0 to 1: how far along its travel the title is at `frame` of `frames`, reaching
    /// the end on the last frame before the postroll.
    fn progress(self, frame: i64, frames: i64) -> f32 {
        let (preroll, postroll) = (i64::from(self.preroll), i64::from(self.postroll));
        let moving = (frames - 1 - preroll - postroll).max(1) as f64;
        let t = ((frame - preroll) as f64).clamp(0.0, moving);
        let (mut ease_in, mut ease_out) = (f64::from(self.ease_in), f64::from(self.ease_out));
        let easing = ease_in + ease_out;
        if easing > moving {
            ease_in *= moving / easing;
            ease_out *= moving / easing;
        }
        // Speeds up and slows down evenly, at the top speed that covers the travel.
        let top = 1.0 / (moving - (ease_in + ease_out) / 2.0);
        let progress = if t < ease_in {
            top * t * t / (2.0 * ease_in)
        } else if t <= moving - ease_out {
            top * (t - ease_in / 2.0)
        } else {
            let left = moving - t;
            1.0 - top * left * left / (2.0 * ease_out)
        };
        progress.clamp(0.0, 1.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roll(scroll: Scroll) -> TitleMotion {
        TitleMotion::Roll(scroll)
    }

    #[test]
    fn a_roll_off_screen_enters_below_and_leaves_above() {
        let motion = roll(Scroll::default());
        assert_eq!(motion.position(300.0, 1080.0, 0, 101), 1080.0);
        assert_eq!(
            motion.position(300.0, 1080.0, 50, 101),
            (1080.0 - 300.0) / 2.0
        );
        assert_eq!(motion.position(300.0, 1080.0, 100, 101), -300.0);
    }

    #[test]
    fn a_long_roll_on_screen_runs_from_its_head_at_the_top_to_its_tail_at_the_bottom() {
        let motion = roll(Scroll {
            start_off_screen: false,
            end_off_screen: false,
            ..Scroll::default()
        });
        assert_eq!(motion.position(3000.0, 1080.0, 0, 11), 0.0);
        assert_eq!(motion.position(3000.0, 1080.0, 10, 11), 1080.0 - 3000.0);
    }

    #[test]
    fn preroll_and_postroll_hold_the_ends() {
        let motion = roll(Scroll {
            preroll: 10,
            postroll: 10,
            ..Scroll::default()
        });
        let at = |frame| motion.position(100.0, 1000.0, frame, 41);
        assert_eq!(at(0), 1000.0);
        assert_eq!(at(10), 1000.0);
        assert_eq!(at(20), (1000.0 - 100.0) / 2.0);
        assert_eq!(at(30), -100.0);
        assert_eq!(at(40), -100.0);
    }

    #[test]
    fn easing_starts_and_stops_slowly_and_still_covers_the_travel() {
        let eased = roll(Scroll {
            ease_in: 20,
            ease_out: 20,
            ..Scroll::default()
        });
        let linear = roll(Scroll::default());
        let travelled =
            |motion: TitleMotion, frame| 1000.0 - motion.position(0.0, 1000.0, frame, 101);
        assert!(travelled(eased, 5) < travelled(linear, 5) / 2.0);
        assert!((travelled(eased, 50) - 500.0).abs() < 0.01);
        assert!(travelled(eased, 95) > travelled(linear, 95));
        assert_eq!(travelled(eased, 100), 1000.0);
    }

    #[test]
    fn a_crawl_right_enters_from_the_left() {
        let motion = TitleMotion::CrawlRight(Scroll::default());
        assert_eq!(motion.position(500.0, 1920.0, 0, 11), -500.0);
        assert_eq!(motion.position(500.0, 1920.0, 10, 11), 1920.0);
    }
}
