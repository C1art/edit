use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use media::StillFormat;
use playback::StillJob;

use crate::editor::Editor;

impl Editor {
    /// A file beside the project named after the open sequence and the playhead's frame.
    pub(crate) fn default_frame_path(&self) -> PathBuf {
        let sequence = self.timeline.sequence();
        let rate = sequence.format.frame_rate;
        let frame = rate.frame_at(self.playhead(), sequence.timebase);
        let project = self.project_path.as_deref();
        let name = self
            .timeline
            .open_nest()
            .and_then(|id| self.timeline.project().media_item(id))
            .map(|item| item.name.clone())
            .or_else(|| {
                project?
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into())
            })
            .unwrap_or_else(|| "Sequence".to_owned());
        let file = format!("{name} {frame:05}.{}", StillFormat::Png.extension());
        project
            .and_then(|path| path.parent())
            .map_or_else(|| PathBuf::from(&file), |folder| folder.join(&file))
    }

    /// Saves the program at the playhead as a PNG, or a TIFF when `path` names one.
    pub(crate) fn export_frame(&mut self, mut path: PathBuf) {
        if self.still_job.is_some() {
            self.notice("A frame is still being exported");
            return;
        }
        let format = StillFormat::of_path(&path).unwrap_or_else(|| {
            path.as_mut_os_string()
                .push(format!(".{}", StillFormat::Png.extension()));
            StillFormat::Png
        });
        let Some(render_state) = &self.render_state else {
            self.notice("Cannot export a frame: there is no GPU renderer");
            return;
        };
        let job = StillJob::start(
            Arc::new(self.timeline.sequence().clone()),
            self.playhead(),
            render_state.device.clone(),
            render_state.queue.clone(),
            path,
            format,
        );
        match job {
            Ok(job) => self.still_job = Some(job),
            Err(error) => self.notice(format!("Cannot export a frame: {error}")),
        }
    }

    pub(crate) fn poll_export_frame(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.still_job else {
            return;
        };
        let Some(outcome) = job.poll() else {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        };
        self.still_job = None;
        match outcome {
            Ok(saved) => {
                for warning in saved.warnings {
                    self.notice(format!("Export frame: {warning}"));
                }
                self.notice(format!("Exported frame {}", saved.path.display()));
            }
            Err(error) => self.notice(format!("Cannot export a frame: {error}")),
        }
    }
}
