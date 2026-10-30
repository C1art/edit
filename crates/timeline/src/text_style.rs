//! A title's look kept under a name, to give other titles the same look.

use serde::{Deserialize, Serialize};

use crate::{Title, TitleMotion};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextStyle {
    pub name: String,
    /// Everything but the words and how they move, which belong to each title.
    look: Title,
}

impl TextStyle {
    pub fn from_title(name: String, title: &Title) -> Self {
        Self {
            name,
            look: Title {
                text: String::new(),
                motion: TitleMotion::Still,
                ..title.clone()
            },
        }
    }

    pub fn apply(&self, title: &mut Title) {
        *title = Title {
            text: std::mem::take(&mut title.text),
            motion: title.motion,
            ..self.look.clone()
        };
    }

    pub fn is_on(&self, title: &Title) -> bool {
        let mut styled = title.clone();
        self.apply(&mut styled);
        styled == *title
    }
}
