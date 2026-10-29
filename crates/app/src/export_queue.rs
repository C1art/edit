use std::sync::Arc;

use egui::{Key, ProgressBar, RichText};
use playback::{ExportEvent, ExportJob, ExportSettings};
use timeline::Sequence;

use crate::editor::Editor;

/// Exports set up earlier, run one after another while editing carries on.
#[derive(Default)]
pub struct ExportQueue {
    items: Vec<Queued>,
    running: Option<(u64, ExportJob)>,
    /// Whether finishing one export starts the next waiting one.
    started: bool,
    next_id: u64,
    pub(crate) open: bool,
}

struct Queued {
    id: u64,
    /// A snapshot, so edits made while waiting don't change what gets exported.
    sequence: Arc<Sequence>,
    settings: ExportSettings,
    summary: String,
    status: Status,
}

enum Status {
    Waiting,
    Exporting { done: i64, total: i64 },
    Done,
    Cancelled,
    Failed(String),
}

impl ExportQueue {
    pub(crate) fn add(
        &mut self,
        sequence: Arc<Sequence>,
        settings: ExportSettings,
        summary: String,
    ) {
        self.next_id += 1;
        self.items.push(Queued {
            id: self.next_id,
            sequence,
            settings,
            summary,
            status: Status::Waiting,
        });
    }

    fn item_mut(&mut self, id: u64) -> Option<&mut Queued> {
        self.items.iter_mut().find(|item| item.id == id)
    }

    fn is_busy(&self) -> bool {
        self.running.is_some()
    }
}

impl Editor {
    pub(crate) fn poll_export_queue(&mut self, ctx: &egui::Context) {
        let mut notices = Vec::new();
        let queue = &mut self.export_queue;
        if let Some((id, job)) = &queue.running {
            let (id, events) = (*id, job.poll());
            let mut ended = None;
            let mut progress = None;
            for event in events {
                match event {
                    ExportEvent::Progress {
                        frames_done,
                        frames_total,
                    } => progress = Some((frames_done, frames_total)),
                    ExportEvent::Warning(error) => notices.push(format!("Export: {error}")),
                    ExportEvent::Finished => ended = Some(Status::Done),
                    ExportEvent::Cancelled => ended = Some(Status::Cancelled),
                    ExportEvent::Failed(error) => ended = Some(Status::Failed(error.to_string())),
                }
            }
            if let Some(item) = queue.item_mut(id) {
                if let Some((done, total)) = progress {
                    item.status = Status::Exporting { done, total };
                }
                if let Some(status) = ended {
                    if matches!(status, Status::Done) {
                        notices.push(format!("Exported {}", item.settings.path.display()));
                    }
                    item.status = status;
                    queue.running = None;
                }
            }
            ctx.request_repaint();
        }
        for notice in notices {
            self.notice(notice);
        }
        if self.export_queue.started && !self.export_queue.is_busy() {
            self.start_next_export();
        }
    }

    fn start_next_export(&mut self) {
        let queue = &mut self.export_queue;
        let Some(item) = queue
            .items
            .iter_mut()
            .find(|item| matches!(item.status, Status::Waiting))
        else {
            queue.started = false;
            self.notice("Export queue finished");
            return;
        };
        let Some(render_state) = &self.render_state else {
            queue.started = false;
            self.notice("Cannot export: there is no GPU renderer");
            return;
        };
        let started = ExportJob::start(
            Arc::clone(&item.sequence),
            render_state.device.clone(),
            render_state.queue.clone(),
            item.settings.clone(),
        );
        match started {
            Ok(job) => {
                item.status = Status::Exporting { done: 0, total: 0 };
                queue.running = Some((item.id, job));
            }
            Err(error) => item.status = Status::Failed(error.to_string()),
        }
    }
}

pub fn window(editor: &mut Editor, ctx: &egui::Context) {
    let queue = &mut editor.export_queue;
    if !queue.open {
        return;
    }
    let mut open = true;
    let mut remove = None;
    crate::dialog::window(ctx, "Export Queue")
        .open(&mut open)
        .default_width(520.0)
        .show(ctx, |ui| {
            if queue.items.is_empty() {
                ui.label(
                    RichText::new("Choose Queue in the Export Media dialog to add exports.").weak(),
                );
            }
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    for item in &queue.items {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                let name = item.settings.path.file_name().map_or_else(
                                    || item.settings.path.display().to_string(),
                                    |name| name.to_string_lossy().into_owned(),
                                );
                                ui.label(RichText::new(name).strong())
                                    .on_hover_text(item.settings.path.display().to_string());
                                ui.label(RichText::new(&item.summary).weak());
                                status_row(ui, &item.status);
                            });
                            let exporting = matches!(item.status, Status::Exporting { .. });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_enabled(!exporting, egui::Button::new("Remove"))
                                        .clicked()
                                    {
                                        remove = Some(item.id);
                                    }
                                },
                            );
                        });
                        ui.separator();
                    }
                });
            ui.horizontal(|ui| {
                let waiting = queue
                    .items
                    .iter()
                    .any(|item| matches!(item.status, Status::Waiting));
                if queue.started {
                    if ui
                        .button("Stop")
                        .on_hover_text("Cancel the export running now")
                        .clicked()
                    {
                        queue.started = false;
                        if let Some((_, job)) = &queue.running {
                            job.cancel();
                        }
                    }
                } else if ui
                    .add_enabled(waiting, egui::Button::new("Start Queue"))
                    .clicked()
                {
                    queue.started = true;
                }
                if ui.button("Clear Finished").clicked() {
                    queue.items.retain(|item| {
                        matches!(item.status, Status::Waiting | Status::Exporting { .. })
                    });
                }
            });
            if ui.input(|input| input.key_pressed(Key::Escape)) {
                queue.open = false;
            }
        });
    if let Some(id) = remove {
        queue.items.retain(|item| item.id != id);
    }
    queue.open &= open;
}

fn status_row(ui: &mut egui::Ui, status: &Status) {
    match status {
        Status::Waiting => {
            ui.label("Waiting");
        }
        Status::Exporting { done, total } => {
            let fraction = if *total > 0 {
                *done as f32 / *total as f32
            } else {
                0.0
            };
            ui.add(
                ProgressBar::new(fraction)
                    .desired_width(300.0)
                    .text(format!("frame {done} of {total}")),
            );
        }
        Status::Done => {
            ui.label("Done");
        }
        Status::Cancelled => {
            ui.label("Cancelled");
        }
        Status::Failed(error) => {
            ui.colored_label(ui.visuals().error_fg_color, format!("Failed: {error}"));
        }
    }
}
