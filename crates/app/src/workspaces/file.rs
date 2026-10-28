//! The workspaces file: a line naming the active workspace, then one line to a
//! workspace with its saved and current layouts, fields split by tabs.

use super::{Layout, Workspace, Workspaces, EDITING};
use crate::effects::SideTab;
use crate::panels::MediaTab;

const ACTIVE: &str = "active";
const WORKSPACE: &str = "workspace";

impl Workspaces {
    pub(super) fn to_text(&self) -> String {
        let mut text = format!("{ACTIVE}\t{}\n", self.list[self.active].name);
        for workspace in &self.list {
            let saved = workspace.saved.to_text();
            let current = workspace.current.to_text();
            text += &format!("{WORKSPACE}\t{}\t{saved}\t{current}\n", workspace.name);
        }
        text
    }

    /// Reads what it can of `text` over the built-in workspaces, skipping lines it
    /// does not understand.
    pub(super) fn from_text(text: &str) -> Self {
        let mut workspaces = Self::default();
        let mut active = None;
        for line in text.lines() {
            match line.split('\t').collect::<Vec<_>>()[..] {
                [ACTIVE, name] => active = Some(name),
                [WORKSPACE, name, saved, current] => {
                    let (Some(saved), Some(current)) =
                        (Layout::from_text(saved), Layout::from_text(current))
                    else {
                        continue;
                    };
                    let kept = workspaces.list.iter_mut().find(|kept| kept.name == name);
                    match kept {
                        Some(kept) => {
                            kept.saved = saved;
                            kept.current = current;
                        }
                        None => workspaces.list.push(Workspace {
                            name: name.to_owned(),
                            saved,
                            current,
                        }),
                    }
                }
                _ => {}
            }
        }
        let active = active.and_then(|name| workspaces.list.iter().position(|w| w.name == name));
        workspaces.active = active.unwrap_or(EDITING);
        workspaces
    }
}

impl Layout {
    fn to_text(self) -> String {
        format!(
            "{} {} {} {} {}",
            self.media_width,
            self.side_width,
            self.timeline_height,
            self.media_tab.words().0,
            self.side_tab.words().0
        )
    }

    fn from_text(text: &str) -> Option<Self> {
        let [media_width, side_width, timeline_height, media_tab, side_tab] =
            text.split(' ').collect::<Vec<_>>()[..]
        else {
            return None;
        };
        let size = |text: &str| text.parse::<f32>().ok().filter(|size| size.is_finite());
        Some(Self {
            media_width: size(media_width)?,
            side_width: size(side_width)?,
            timeline_height: size(timeline_height)?,
            media_tab: MediaTab::ALL
                .into_iter()
                .find(|tab| tab.words().0 == media_tab)?,
            side_tab: SideTab::ALL
                .into_iter()
                .find(|tab| tab.words().0 == side_tab)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspaces_round_trip_with_changes_and_new_ones() {
        let mut workspaces = Workspaces::default();
        workspaces.list[0].current.media_width = 512.5;
        workspaces.list[0].current.side_tab = SideTab::Scopes;
        let layout = Layout::new([300.0, 400.0, 200.0], MediaTab::Markers, SideTab::ClipMixer);
        workspaces.list.push(Workspace {
            name: "My Mix".to_owned(),
            saved: layout,
            current: Layout {
                timeline_height: 250.0,
                ..layout
            },
        });
        workspaces.active = workspaces.list.len() - 1;

        assert_eq!(Workspaces::from_text(&workspaces.to_text()), workspaces);
    }

    #[test]
    fn unreadable_lines_leave_the_built_in_workspaces() {
        let text = "active\tGone\n\
                    workspace\tEditing\t1 2 3 project nowhere\t1 2 3 project scopes\n\
                    workspace\tShort\t1 2 project scopes\t1 2 3 project scopes\n\
                    something else\n";

        assert_eq!(Workspaces::from_text(text), Workspaces::default());
    }
}
