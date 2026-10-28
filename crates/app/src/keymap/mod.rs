mod command;
mod defaults;
mod window;

use std::fmt;

use egui::{Event, Key, Modifiers};

pub use command::Command;
pub use window::{window, KeymapWindow};

/// A key with the modifiers held, Ctrl standing for Command on a Mac.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: Key,
}

impl Chord {
    pub const fn new(ctrl: bool, shift: bool, alt: bool, key: Key) -> Self {
        Self {
            ctrl,
            shift,
            alt,
            key,
        }
    }

    fn held(modifiers: Modifiers, key: Key) -> Self {
        let ctrl = modifiers.ctrl || modifiers.command || modifiers.mac_cmd;
        Self::new(ctrl, modifiers.shift, modifiers.alt, key)
    }

    /// The chord `event` presses, if any. The windowing layer turns clipboard chords
    /// into their own events, so those are read back into the keys behind them.
    pub fn pressed(event: &Event, modifiers: Modifiers) -> Option<Self> {
        let ctrl = modifiers.ctrl || modifiers.command || modifiers.mac_cmd;
        match event {
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if !is_modifier(*key) => Some(Self::held(*modifiers, *key)),
            Event::Copy => Some(Self::new(true, false, false, Key::C)),
            // Shift+Delete is also a cut to the windowing layer.
            Event::Cut if modifiers.shift && !ctrl => {
                Some(Self::new(false, true, false, Key::Delete))
            }
            Event::Cut => Some(Self::new(true, modifiers.shift, false, Key::X)),
            Event::Paste(_) => Some(Self::new(true, modifiers.shift && ctrl, false, Key::V)),
            _ => None,
        }
    }

    /// Reads the form `Display` writes in the shortcuts file, such as `Ctrl+Shift+Comma`.
    pub fn parse(text: &str) -> Option<Self> {
        let mut chord = Self::new(false, false, false, Key::A);
        let mut rest = text.trim();
        loop {
            let flag = if let Some(after) = rest.strip_prefix("Ctrl+") {
                rest = after;
                &mut chord.ctrl
            } else if let Some(after) = rest.strip_prefix("Shift+") {
                rest = after;
                &mut chord.shift
            } else if let Some(after) = rest.strip_prefix("Alt+") {
                rest = after;
                &mut chord.alt
            } else {
                break;
            };
            *flag = true;
        }
        chord.key = Key::from_name(rest)?;
        Some(chord)
    }

    /// As shown in menus, with symbols for punctuation, where the file spells it out.
    pub fn label(self) -> String {
        let key = match self.key {
            Key::Quote => "'",
            key => key.symbol_or_name(),
        };
        format!("{}{key}", self.modifier_prefix())
    }

    fn modifier_prefix(self) -> String {
        [
            (self.ctrl, "Ctrl+"),
            (self.shift, "Shift+"),
            (self.alt, "Alt+"),
        ]
        .into_iter()
        .filter_map(|(held, name)| held.then_some(name))
        .collect()
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}{}", self.modifier_prefix(), self.key.name())
    }
}

/// Modifier keys arrive as presses of their own, but only ever hold others down.
fn is_modifier(key: Key) -> bool {
    use Key::*;
    matches!(
        key,
        ShiftLeft
            | ShiftRight
            | ControlLeft
            | ControlRight
            | AltLeft
            | AltRight
            | SuperLeft
            | SuperRight
    )
}

/// Takes the first key press this frame that `pick` accepts out of the input, so no
/// widget acts on it too.
pub fn take_press<T>(ctx: &egui::Context, mut pick: impl FnMut(Chord) -> Option<T>) -> Option<T> {
    ctx.input_mut(|input| {
        let modifiers = input.modifiers;
        let (index, picked) = input.events.iter().enumerate().find_map(|(index, event)| {
            let picked = pick(Chord::pressed(event, modifiers)?)?;
            Some((index, picked))
        })?;
        input.events.remove(index);
        Some(picked)
    })
}

/// Which keys run which commands.
#[derive(Debug, Clone, PartialEq)]
pub struct Keymap {
    bindings: Vec<(Command, Vec<Chord>)>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self {
            bindings: Command::ALL
                .into_iter()
                .map(|command| (command, command.defaults()))
                .collect(),
        }
    }
}

impl Keymap {
    pub fn chords(&self, command: Command) -> &[Chord] {
        self.bindings
            .iter()
            .find(|(bound, _)| *bound == command)
            .map_or(&[], |(_, chords)| chords)
    }

    /// The first key for `command` as menus show it; empty when it has none.
    pub fn label(&self, command: Command) -> String {
        self.chords(command)
            .first()
            .map_or_else(String::new, |chord| chord.label())
    }

    /// The command `chord` runs. Punctuation typed with Shift, such as `+`, also reaches
    /// a binding without it when nothing is bound with Shift.
    pub fn command(&self, chord: Chord) -> Option<Command> {
        let symbol = chord.key.symbol_or_name();
        let punctuation =
            symbol.len() == 1 && symbol.bytes().all(|byte| byte.is_ascii_punctuation());
        self.exact(chord).or_else(|| {
            (chord.shift && punctuation)
                .then(|| {
                    self.exact(Chord {
                        shift: false,
                        ..chord
                    })
                })
                .flatten()
        })
    }

    fn exact(&self, chord: Chord) -> Option<Command> {
        self.bindings
            .iter()
            .find(|(_, chords)| chords.contains(&chord))
            .map(|(command, _)| *command)
    }

    /// Binds `chord` to `command`, taking it from the command that had it, which is
    /// returned.
    pub fn bind(&mut self, command: Command, chord: Chord) -> Option<Command> {
        let previous = self.exact(chord).filter(|&owner| owner != command);
        for (bound, chords) in &mut self.bindings {
            if *bound == command {
                if !chords.contains(&chord) {
                    chords.push(chord);
                }
            } else {
                chords.retain(|&taken| taken != chord);
            }
        }
        previous
    }

    pub fn unbind(&mut self, command: Command, chord: Chord) {
        if let Some((_, chords)) = self
            .bindings
            .iter_mut()
            .find(|(bound, _)| *bound == command)
        {
            chords.retain(|&bound| bound != chord);
        }
    }

    /// The shortcuts file: a line per command, its id, a colon, then its keys separated
    /// by commas.
    pub fn to_text(&self) -> String {
        let mut text = String::from("# Keyboard shortcuts: command: keys, separated by commas\n");
        for (command, chords) in &self.bindings {
            let keys: Vec<String> = chords.iter().map(Chord::to_string).collect();
            text += &format!("{}: {}\n", command.id(), keys.join(", "));
        }
        text
    }

    /// Reads a shortcuts file. Commands it leaves out keep their defaults, so ones added
    /// since it was written still have keys; lines it can't read are skipped.
    pub fn from_text(text: &str) -> Self {
        let mut keymap = Self::default();
        let lines = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'));
        for line in lines {
            let Some((id, keys)) = line.split_once(':') else {
                continue;
            };
            let Some(command) = Command::from_id(id.trim()) else {
                continue;
            };
            let chords = keys
                .split(',')
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .filter_map(Chord::parse)
                .collect();
            if let Some((_, bound)) = keymap
                .bindings
                .iter_mut()
                .find(|(bound, _)| *bound == command)
            {
                *bound = chords;
            }
        }
        keymap
    }

    /// Commands other than `command` that share one of its keys, which only a hand-edited
    /// file can cause.
    pub fn clashes(&self, command: Command, chord: Chord) -> bool {
        self.bindings
            .iter()
            .any(|(other, chords)| *other != command && chords.contains(&chord))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(text: &str) -> Chord {
        Chord::parse(text).unwrap_or_else(|| panic!("unreadable chord {text}"))
    }

    #[test]
    fn no_two_commands_share_a_default_key() {
        let keymap = Keymap::default();
        for command in Command::ALL {
            for &key in keymap.chords(command) {
                assert!(!keymap.clashes(command, key), "{command:?} shares {key}");
            }
        }
    }

    #[test]
    fn a_customized_keymap_survives_its_file() {
        let mut keymap = Keymap::default();
        keymap.bind(Command::Save, chord("Ctrl+Shift+Alt+Comma"));
        keymap.unbind(Command::MarkIn, chord("I"));
        keymap.bind(Command::ZoomIn, chord("Shift+F5"));
        assert_eq!(Keymap::from_text(&keymap.to_text()), keymap);
    }

    #[test]
    fn a_file_missing_commands_or_holding_unknown_ones_keeps_the_rest() {
        let text = "renamed_long_ago: Ctrl+Q\nsave: Ctrl+Nonsense, F2\nmark_in:\n";
        let keymap = Keymap::from_text(text);
        assert_eq!(keymap.chords(Command::Save), [chord("F2")]);
        assert!(keymap.chords(Command::MarkIn).is_empty());
        assert_eq!(keymap.chords(Command::Undo), Command::Undo.defaults());
    }

    #[test]
    fn binding_a_taken_key_moves_it() {
        let mut keymap = Keymap::default();
        let previous = keymap.bind(Command::MatchFrame, chord("Ctrl+K"));
        assert_eq!(previous, Some(Command::AddEdit));
        assert_eq!(keymap.command(chord("Ctrl+K")), Some(Command::MatchFrame));
        assert!(keymap.chords(Command::AddEdit).is_empty());
    }

    #[test]
    fn modifiers_must_match_but_shifted_symbols_fall_back() {
        let keymap = Keymap::default();
        assert_eq!(
            keymap.command(chord("A")),
            Some(Command::TrackSelectForwardTool)
        );
        assert_eq!(
            keymap.command(chord("Shift+A")),
            Some(Command::TrackSelectBackwardTool)
        );
        assert_eq!(keymap.command(chord("Shift+Equals")), Some(Command::ZoomIn));
        assert_eq!(keymap.command(chord("Alt+A")), None);
        assert_eq!(keymap.command(chord("Ctrl+Shift+K")), None);
    }
}
