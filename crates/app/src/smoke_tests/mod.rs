//! Drives the whole editor headlessly, through eframe's test constructors, to find what
//! takes it down.

mod bench_clips;
mod commands;
mod export_speed;
mod full_screen;
mod large_project;
mod layout;
mod monkey;
mod playback_speed;
mod soak;
mod timing;
mod transcription;
mod usage;
mod vst3_plugin;

use std::future::Future;
use std::path::PathBuf;
use std::sync::{Once, OnceLock};
use std::task::{Context, Poll, Waker};

use eframe::egui_wgpu;
use timeline::{FrameRate, ImportMedia, PlaceClip, PlaceMode, Ticks, VideoFormat};

use crate::editor::Editor;
use crate::window_state::WindowState;

pub(super) const FRAME: i64 = 90_000 / 25;
pub(super) const SCREEN: egui::Vec2 = egui::vec2(1600.0, 900.0);

pub(super) fn editor(gpu: Option<&egui_wgpu::RenderState>) -> Editor {
    static SANDBOX: Once = Once::new();
    SANDBOX.call_once(|| {
        let folder = std::env::temp_dir().join(format!("edit-smoke-{}", std::process::id()));
        // Whatever the editor saves, exports or browses to by default stays in here. Set
        // before any editor reads them, and no other test in this crate reads them.
        std::env::set_var("APPDATA", folder.join("appdata"));
        std::env::set_var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }, &folder);
    });
    let mut creation = eframe::CreationContext::_new_kittest(egui::Context::default());
    creation.wgpu_render_state = gpu.cloned();
    Editor::new(&creation, Vec::new(), WindowState::default())
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::yield_now();
    }
}

pub(super) fn gpu() -> egui_wgpu::RenderState {
    // One for the whole test binary, as AMD's Vulkan driver can fail, or crash, while
    // two threads set up or use devices of their own at once.
    static GPU: OnceLock<egui_wgpu::RenderState> = OnceLock::new();
    GPU.get_or_init(|| {
        let config = crate::gpu_setup::wgpu_configuration();
        let instance = block_on(config.wgpu_setup.new_instance());
        block_on(egui_wgpu::RenderState::create(
            &config,
            &instance,
            None,
            egui_wgpu::RendererOptions::default(),
        ))
        .expect("a GPU")
    })
    .clone()
}

/// The window the editor draws into, with a clock that moves a frame at a time.
pub(super) struct Screen {
    frame: eframe::Frame,
    time: f64,
    pub(super) size: egui::Vec2,
}

impl Default for Screen {
    fn default() -> Self {
        Self {
            frame: eframe::Frame::_new_kittest(),
            time: 0.0,
            size: SCREEN,
        }
    }
}

impl Screen {
    const FRAME_SECONDS: f32 = 1.0 / 60.0;

    pub(super) fn draw(&mut self, editor: &mut Editor, events: Vec<egui::Event>) {
        let mut output = self.run(editor, events);
        output.textures_delta.clear();
    }

    /// Draws a frame and hands back what egui made of it, to be tessellated as a window
    /// would.
    pub(super) fn run(
        &mut self,
        editor: &mut Editor,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        self.time += f64::from(Self::FRAME_SECONDS);
        let modifiers = events
            .iter()
            .find_map(|event| match event {
                egui::Event::Key { modifiers, .. }
                | egui::Event::PointerButton { modifiers, .. }
                | egui::Event::MouseWheel { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or_default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.time),
            predicted_dt: Self::FRAME_SECONDS,
            events: std::iter::once(egui::Event::ModifiersChanged(modifiers))
                .chain(events)
                .collect(),
            ..Default::default()
        };
        let ctx = editor.ctx.clone();
        ctx.run_ui(input, |ui| eframe::App::ui(editor, ui, &mut self.frame))
    }
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/flash_beep_1s_25fps_44k.mp4")
}

pub(super) fn import(editor: &mut Editor, frames: i64) {
    let format = VideoFormat {
        width: 64,
        height: 36,
        frame_rate: FrameRate::FPS_25,
        ..VideoFormat::default()
    };
    editor.apply(ImportMedia {
        name: "flash".into(),
        path: fixture(),
        duration: Ticks(frames * FRAME),
        video: Some(format),
        has_audio: true,
        still: false,
        interpreted_rate: None,
        placed: None,
        frame_fit: None,
        timecode: None,
    });
}

pub(super) fn place(editor: &mut Editor, at: i64, frames: i64) {
    let Some(source) = editor
        .timeline
        .sequence()
        .media
        .first()
        .map(|media| media.id)
    else {
        return;
    };
    editor.apply(PlaceClip {
        source,
        source_in: Ticks::ZERO,
        duration: Ticks(frames * FRAME),
        at: Ticks(at * FRAME),
        video_track: Some(0),
        audio_track: Some(0),
        mode: PlaceMode::Overwrite,
        fill: None,
    });
}

pub(super) fn message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_default()
}
