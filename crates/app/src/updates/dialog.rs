use std::time::Duration;

use crossbeam::channel::TryRecvError;
use egui::{Context, Key, ScrollArea, Ui, ViewportCommand};

use super::{offer, Asked, Checked, State, Updates};
use crate::editor::Editor;

/// How often to look in on a check or download, which don't wake the window themselves.
const POLL: Duration = Duration::from_millis(250);

pub(crate) fn help_menu_item(editor: &mut Editor, ui: &mut Ui) {
    let updates = &mut editor.updates;
    let label = match &updates.state {
        State::Checking { .. } => "Checking for Updates…".to_owned(),
        State::Downloading { percent, .. } => format!("Downloading Update ({percent}%)"),
        State::Ready { .. } => "Restart to Update…".to_owned(),
        State::Idle | State::Offered(_) => "Check for Updates…".to_owned(),
    };
    if ui.button(label).clicked() {
        match &mut updates.state {
            State::Idle => updates.check(Asked::FromMenu),
            State::Ready { asking, .. } => *asking = true,
            State::Checking { .. } | State::Offered(_) | State::Downloading { .. } => {}
        }
    }
}

pub(crate) fn ui(editor: &mut Editor, ctx: &Context) {
    if let Some(notice) = poll(&mut editor.updates, ctx) {
        editor.notice(notice);
    }
    match &editor.updates.state {
        State::Offered(_) => offer_dialog(&mut editor.updates, ctx),
        State::Ready { asking: true, .. } => restart_dialog(&mut editor.updates, ctx),
        _ => {}
    }
}

/// Takes in what the workers have done; returns anything the user should be told.
fn poll(updates: &mut Updates, ctx: &Context) -> Option<String> {
    match &mut updates.state {
        State::Checking { asked, reply } => {
            let asked = *asked;
            let checked = match reply.try_recv() {
                Ok(checked) => checked,
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(POLL);
                    return None;
                }
                Err(TryRecvError::Disconnected) => Err("the check couldn't start".to_owned()),
            };
            updates.state = State::Idle;
            let asked_for = asked == Asked::FromMenu;
            match checked {
                Ok(Checked::Available(info)) => {
                    let version = &info.TargetFullRelease.Version;
                    if offer(asked, version, updates.skipped.as_deref()) {
                        updates.state = State::Offered(info);
                    }
                    None
                }
                Ok(Checked::UpToDate(version)) => {
                    asked_for.then(|| format!("Cledit {version} is the latest version."))
                }
                Ok(Checked::NotInstalled) => asked_for
                    .then(|| "Updates work only in Cledit installed from its installer.".into()),
                Err(error) => asked_for.then(|| format!("Couldn't check for updates: {error}")),
            }
        }
        State::Downloading {
            info,
            percent,
            progress,
            done,
        } => {
            if let Some(latest) = progress.try_iter().last() {
                *percent = latest;
            }
            let result = match done.try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(POLL);
                    return None;
                }
                Err(TryRecvError::Disconnected) => Err("the download couldn't start".to_owned()),
            };
            let info = std::mem::take(info);
            match result {
                Ok(()) => {
                    updates.state = State::Ready { info, asking: true };
                    None
                }
                Err(error) => {
                    updates.state = State::Idle;
                    Some(format!("Couldn't download the update: {error}"))
                }
            }
        }
        State::Idle | State::Offered(_) | State::Ready { .. } => None,
    }
}

enum Answer {
    Update,
    Later,
    Skip,
}

fn offer_dialog(updates: &mut Updates, ctx: &Context) {
    let State::Offered(info) = &updates.state else {
        return;
    };
    let release = &info.TargetFullRelease;
    let mut answer = None;
    crate::dialog::window(ctx, "Update Available").show(ctx, |ui| {
        ui.label(format!(
            "Cledit {} is available. You have {}.",
            release.Version,
            env!("CARGO_PKG_VERSION")
        ));
        let notes = release.NotesMarkdown.trim();
        if !notes.is_empty() {
            ui.add_space(4.0);
            ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                ui.label(notes);
            });
        }
        crate::dialog::footer(ui, |ui| {
            if ui.button("Skip This Version").clicked() {
                answer = Some(Answer::Skip);
            }
            if ui.button("Ask Later").clicked() || ui.input(|input| input.key_pressed(Key::Escape))
            {
                answer = Some(Answer::Later);
            }
            if ui.add(crate::dialog::primary("Update")).clicked() {
                answer = Some(Answer::Update);
            }
        });
    });
    let Some(answer) = answer else {
        return;
    };
    let State::Offered(info) = std::mem::take(&mut updates.state) else {
        return;
    };
    match answer {
        Answer::Update => {
            let (progress, done) = super::worker::download(&info);
            updates.state = State::Downloading {
                info,
                percent: 0,
                progress,
                done,
            };
        }
        Answer::Later => {}
        Answer::Skip => updates.skipped = Some(info.TargetFullRelease.Version.clone()),
    }
}

fn restart_dialog(updates: &mut Updates, ctx: &Context) {
    let State::Ready { info, asking } = &mut updates.state else {
        return;
    };
    let mut restart = false;
    crate::dialog::window(ctx, "Update Ready").show(ctx, |ui| {
        ui.label(format!(
            "Cledit {} is ready. Restart now to use it, or it is installed when you quit.",
            info.TargetFullRelease.Version
        ));
        crate::dialog::footer(ui, |ui| {
            if ui.button("Later").clicked() || ui.input(|input| input.key_pressed(Key::Escape)) {
                *asking = false;
            }
            if ui.add(crate::dialog::primary("Restart Now")).clicked() {
                *asking = false;
                restart = true;
            }
        });
    });
    if restart {
        // Closing asks about unsaved work and running exports first, as any quit does.
        updates.restart = true;
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
}
