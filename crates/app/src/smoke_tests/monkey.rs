//! Clicks, drags, scrolls, types and presses keys all over the window at random, from a
//! timeline with something on it, and reports the gestures leading up to any panic.

use std::ops::Range;
use std::panic::{catch_unwind, AssertUnwindSafe};

use eframe::egui_wgpu;
use egui::{Event, Key, Modifiers, PointerButton, Pos2};

use super::{editor, gpu, import, message, place, Screen, SCREEN};
use crate::editor::Editor;
use crate::keymap::Command;

const GESTURES: usize = 600;
/// Gestures shown before a panic, to retrace it.
const SHOWN: usize = 12;
const MENU_BAR_HEIGHT: f32 = 24.0;

const KEYS: [Key; 32] = [
    Key::Space,
    Key::Enter,
    Key::Escape,
    Key::Delete,
    Key::Backspace,
    Key::Tab,
    Key::ArrowLeft,
    Key::ArrowRight,
    Key::ArrowUp,
    Key::ArrowDown,
    Key::Home,
    Key::End,
    Key::PageUp,
    Key::PageDown,
    Key::A,
    Key::C,
    Key::D,
    Key::E,
    Key::I,
    Key::J,
    Key::K,
    Key::L,
    Key::M,
    Key::O,
    Key::Q,
    Key::V,
    Key::W,
    Key::X,
    Key::Z,
    Key::Comma,
    Key::Period,
    Key::Plus,
];

/// Typed into whatever field has focus: numbers out of range, timecode-like and not.
const TEXTS: [&str; 8] = [
    "0",
    "-1",
    "99999999999",
    "00:00:00:00",
    "1e309",
    "abc",
    "",
    "12.5",
];

/// SplitMix64: a fixed seed replays the same gestures.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())]
    }
}

#[derive(Debug, Clone)]
enum Gesture {
    Click {
        at: Pos2,
        button: PointerButton,
        modifiers: Modifiers,
    },
    DoubleClick {
        at: Pos2,
    },
    Drag {
        from: Pos2,
        to: Pos2,
        modifiers: Modifiers,
    },
    Scroll {
        at: Pos2,
        delta: egui::Vec2,
        modifiers: Modifiers,
    },
    Key {
        key: Key,
        modifiers: Modifiers,
    },
    Type(&'static str),
}

struct Monkey {
    dice: Dice,
    /// Menus and popups open near the last click, so later clicks often land nearby.
    last: Pos2,
}

impl Monkey {
    fn place(&mut self) -> Pos2 {
        let roll = self.dice.below(100);
        let pos = if roll < 15 {
            Pos2::new(
                self.dice.unit() * SCREEN.x * 0.5,
                self.dice.unit() * MENU_BAR_HEIGHT,
            )
        } else if roll < 45 {
            self.last
                + egui::vec2(
                    (self.dice.unit() - 0.3) * 240.0,
                    (self.dice.unit() - 0.2) * 400.0,
                )
        } else {
            Pos2::new(self.dice.unit() * SCREEN.x, self.dice.unit() * SCREEN.y)
        };
        let pos = pos.clamp(Pos2::ZERO, Pos2::ZERO + SCREEN);
        self.last = pos;
        pos
    }

    fn modifiers(&mut self) -> Modifiers {
        if self.dice.chance(75) {
            return Modifiers::NONE;
        }
        Modifiers {
            alt: self.dice.chance(30),
            ctrl: self.dice.chance(50),
            shift: self.dice.chance(40),
            command: false,
            mac_cmd: false,
        }
    }

    fn gesture(&mut self) -> Gesture {
        match self.dice.below(100) {
            0..=44 => Gesture::Click {
                at: self.place(),
                button: if self.dice.chance(20) {
                    PointerButton::Secondary
                } else {
                    PointerButton::Primary
                },
                modifiers: self.modifiers(),
            },
            45..=51 => Gesture::DoubleClick { at: self.place() },
            52..=69 => {
                let from = self.place();
                Gesture::Drag {
                    from,
                    to: self.place(),
                    modifiers: self.modifiers(),
                }
            }
            70..=76 => Gesture::Scroll {
                at: self.place(),
                delta: egui::vec2(0.0, (self.dice.unit() - 0.5) * 400.0),
                modifiers: self.modifiers(),
            },
            77..=91 => Gesture::Key {
                key: self.dice.pick(&KEYS),
                modifiers: self.modifiers(),
            },
            _ => Gesture::Type(self.dice.pick(&TEXTS)),
        }
    }
}

fn button(pos: Pos2, button: PointerButton, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers,
    }
}

/// The events of each frame the gesture spans.
fn frames(gesture: &Gesture) -> Vec<Vec<Event>> {
    match *gesture {
        Gesture::Click {
            at,
            button: which,
            modifiers,
        } => vec![
            vec![Event::PointerMoved(at)],
            vec![button(at, which, true, modifiers)],
            vec![button(at, which, false, modifiers)],
        ],
        Gesture::DoubleClick { at } => {
            let click = |pressed| button(at, PointerButton::Primary, pressed, Modifiers::NONE);
            vec![
                vec![Event::PointerMoved(at)],
                vec![click(true), click(false)],
                vec![click(true), click(false)],
            ]
        }
        Gesture::Drag {
            from,
            to,
            modifiers,
        } => {
            let primary = |at, pressed| button(at, PointerButton::Primary, pressed, modifiers);
            let mut frames = vec![vec![Event::PointerMoved(from)], vec![primary(from, true)]];
            frames.extend(
                (1..=5).map(|step| vec![Event::PointerMoved(from.lerp(to, step as f32 / 5.0))]),
            );
            frames.push(vec![primary(to, false)]);
            frames
        }
        Gesture::Scroll {
            at,
            delta,
            modifiers,
        } => vec![
            vec![Event::PointerMoved(at)],
            vec![Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta,
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Wheel,
                modifiers,
            }],
        ],
        Gesture::Key { key, modifiers } => {
            let press = |pressed| Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            };
            vec![vec![press(true)], vec![press(false)]]
        }
        Gesture::Type(text) => vec![vec![Event::Text(text.to_owned())]],
    }
}

fn crowded(editor: &mut Editor) {
    import(editor, 25);
    place(editor, 0, 10);
    place(editor, 14, 11);
    crate::shortcuts::run(editor, Command::NewTitle);
}

fn run_monkey(gpu: Option<&egui_wgpu::RenderState>, seeds: Range<u64>) {
    let mut panics = Vec::new();
    for seed in seeds {
        let mut editor = editor(gpu);
        let mut screen = Screen::default();
        crowded(&mut editor);
        let mut monkey = Monkey {
            dice: Dice(seed),
            last: Pos2::ZERO,
        };
        let mut done: Vec<Gesture> = Vec::new();
        for step in 0..GESTURES {
            let gesture = monkey.gesture();
            let ran = catch_unwind(AssertUnwindSafe(|| {
                for events in frames(&gesture) {
                    screen.draw(&mut editor, events);
                }
                screen.draw(&mut editor, Vec::new());
            }));
            done.push(gesture);
            if let Err(payload) = ran {
                let recent = &done[done.len().saturating_sub(SHOWN)..];
                panics.push(format!(
                    "seed {seed}, gesture {step}: {}\n  after {recent:?}",
                    message(&*payload)
                ));
                break;
            }
        }
    }
    assert!(panics.is_empty(), "\n{}", panics.join("\n"));
}

/// `EDIT_MONKEY_SEEDS=0..200` runs longer than the default.
fn seeds(default: Range<u64>) -> Range<u64> {
    std::env::var("EDIT_MONKEY_SEEDS")
        .ok()
        .and_then(|seeds| {
            let (start, end) = seeds.split_once("..")?;
            Some(start.parse().ok()?..end.parse().ok()?)
        })
        .unwrap_or(default)
}

#[test]
#[ignore = "slow; decodes a real file with FFmpeg"]
fn random_gestures_never_panic() {
    run_monkey(None, seeds(0..8));
}

#[test]
#[ignore = "slow; needs a GPU, and decodes a real file with FFmpeg"]
fn random_gestures_never_panic_with_a_gpu() {
    run_monkey(Some(&gpu()), seeds(100..104));
}
