//! Shows a plugin's editor in a Win32 window on a thread of its own.

mod frame;
mod handler;
mod win32;
mod window;

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::Arc;

use vst3::Steinberg::Vst::{IComponentHandler, IEditControllerTrait};
use vst3::Steinberg::{kPlatformTypeHWND, kResultTrue, IPlugFrame, IPlugViewTrait, ViewRect};
use vst3::{ComPtr, ComWrapper};

use self::frame::Frame;
use self::handler::Handler;
use self::window::Window;
use super::check;
use crate::editor::{Command, EditorEvent, EditorSetup};
use crate::{Plugin, PluginError};

/// The editor's instance never plays, so these only need to be valid.
const SAMPLE_RATE: u32 = 48_000;
const MAX_BLOCK: usize = 512;

pub(crate) fn spawn(
    setup: EditorSetup,
    commands: Receiver<Command>,
    events: Sender<EditorEvent>,
    window: Arc<AtomicIsize>,
) -> Result<(), PluginError> {
    std::thread::Builder::new()
        .name("plugin editor".to_owned())
        .spawn(move || {
            let event = match run(&setup, &commands, &events, &window) {
                Ok(state) => EditorEvent::Closed(state),
                Err(error) => EditorEvent::Failed(error),
            };
            if events.send(event).is_ok() {
                (setup.notify)();
            }
        })
        .map(drop)
        .map_err(|_| PluginError::Editor("starting the editor's thread"))
}

pub(crate) fn wake(window: isize) {
    if window != 0 {
        window::post_wake(window);
    }
}

/// Shows the editor until it is closed; the plugin's state then.
fn run(
    setup: &EditorSetup,
    commands: &Receiver<Command>,
    events: &Sender<EditorEvent>,
    window: &AtomicIsize,
) -> Result<Option<Vec<u8>>, PluginError> {
    let mut plugin = Plugin::load(&setup.path, setup.class_id, SAMPLE_RATE, MAX_BLOCK)?;
    if !setup.state.is_empty() {
        plugin.set_state(&setup.state)?;
    }
    for &(id, value) in &setup.params {
        // A parameter the plugin no longer has is left out, as playback does.
        let _ = plugin.set_parameter(id, value);
    }
    let handler = ComWrapper::new(Handler::new(events.clone(), Arc::clone(&setup.notify)));
    let handler_ptr = handler
        .to_com_ptr::<IComponentHandler>()
        .ok_or(PluginError::MissingInterface("IComponentHandler"))?;
    set_component_handler(&plugin, handler_ptr.as_ptr());
    let shown = show(&mut plugin, &handler, setup, commands, window);
    set_component_handler(&plugin, ptr::null_mut());
    shown?;
    Ok(plugin.state().ok())
}

fn set_component_handler(plugin: &Plugin, handler: *mut IComponentHandler) {
    // SAFETY: `handler` is null or a live handler that `run` keeps alive until it is
    // cleared again here. The result is ignored: a controller that refuses it simply
    // reports no edits.
    unsafe { plugin.instance.controller.setComponentHandler(handler) };
}

fn show(
    plugin: &mut Plugin,
    handler: &Handler,
    setup: &EditorSetup,
    commands: &Receiver<Command>,
    published: &AtomicIsize,
) -> Result<(), PluginError> {
    let view = plugin.instance.create_view().ok_or(PluginError::NoEditor)?;
    // SAFETY: the platform type is a NUL-terminated string constant.
    if unsafe { view.isPlatformTypeSupported(kPlatformTypeHWND) } != kResultTrue {
        return Err(PluginError::NoEditor);
    }
    let mut size = ViewRect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: `size` is a writable rect. An editor that can't say yet leaves it empty.
    unsafe { view.getSize(&mut size) };
    // SAFETY: plain query on a live view.
    let resizable = unsafe { view.canResize() } == kResultTrue;
    let window = Window::create(&setup.title, &size, resizable)?;
    let frame = ComWrapper::new(Frame::new(window.handle(), window.style()))
        .to_com_ptr::<IPlugFrame>()
        .ok_or(PluginError::MissingInterface("IPlugFrame"))?;
    attach(&view, &frame, &window)?;
    window::set_view(Some(view.clone()));
    published.store(window.handle(), Ordering::Release);
    window.show();
    while window.pump() && tend(plugin, handler, commands) {}
    published.store(0, Ordering::Release);
    window::set_view(None);
    // SAFETY: the view is attached; the spec has it removed before its window goes and
    // its frame cleared before release.
    unsafe {
        view.removed();
        view.setFrame(ptr::null_mut());
    }
    Ok(())
}

fn attach(
    view: &ComPtr<vst3::Steinberg::IPlugView>,
    frame: &ComPtr<IPlugFrame>,
    window: &Window,
) -> Result<(), PluginError> {
    // SAFETY: `frame` outlives the view's attachment: `show` clears it before dropping it.
    unsafe { view.setFrame(frame.as_ptr()) };
    // SAFETY: the handle is a live window owned by this thread, of the type named.
    let attached = unsafe { view.attached(window.handle() as *mut c_void, kPlatformTypeHWND) };
    let attached = check(attached, "IPlugView::attached");
    if attached.is_err() {
        // SAFETY: as above; the failed view keeps no frame.
        unsafe { view.setFrame(ptr::null_mut()) };
    }
    attached
}

/// Applies the editor's edits to its own instance and the host's commands to the editor;
/// false once the host asks to close.
fn tend(plugin: &mut Plugin, handler: &Handler, commands: &Receiver<Command>) -> bool {
    let edits = handler.take();
    for (id, value) in edits.values {
        // The editor has set the controller already; this queues the value for the
        // processor so the state saved at close includes it.
        let _ = plugin.set_parameter(id, value);
    }
    if edits.reloaded || edits.settled {
        if let Ok(state) = plugin.state() {
            let event = if edits.reloaded {
                let values = plugin
                    .parameters()
                    .iter()
                    .map(|info| (info.id, plugin.parameter(info.id)))
                    .collect();
                EditorEvent::Reloaded { state, values }
            } else {
                EditorEvent::State(state)
            };
            handler.send(event);
        }
    }
    loop {
        match commands.try_recv() {
            Ok(Command::SetParameter(id, value)) => {
                let _ = plugin.set_parameter(id, value);
            }
            Ok(Command::Close) | Err(TryRecvError::Disconnected) => return false,
            Err(TryRecvError::Empty) => return true,
        }
    }
}
