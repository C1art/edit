use std::cell::{Cell, RefCell};
use std::ptr;
use std::sync::OnceLock;

use vst3::ComPtr;
use vst3::Steinberg::{IPlugView, IPlugViewTrait, ViewRect};

use super::win32::*;
use crate::PluginError;

const CLASS_NAME: &str = "EditPluginEditor";
/// Size for an editor that doesn't say before it is attached.
const FALLBACK_SIZE: (i32, i32) = (640, 480);
/// Wakes the message loop now and then in case a wake-up was posted before the window
/// handle was published.
const BACKSTOP_MS: u32 = 100;

thread_local! {
    static VIEW: RefCell<Option<ComPtr<IPlugView>>> = const { RefCell::new(None) };
    static CLOSE_REQUESTED: Cell<bool> = const { Cell::new(false) };
    /// Set while the window follows the plugin's own resize, which mustn't echo back.
    static PLUGIN_RESIZING: Cell<bool> = const { Cell::new(false) };
}

/// A top-level window for one editor on this thread, destroyed on drop.
pub(super) struct Window {
    handle: Hwnd,
    style: u32,
}

impl Window {
    pub(super) fn create(
        title: &str,
        size: &ViewRect,
        resizable: bool,
    ) -> Result<Self, PluginError> {
        register_class()?;
        let style = if resizable {
            WS_OVERLAPPEDWINDOW
        } else {
            WS_OVERLAPPEDWINDOW & !(WS_THICKFRAME | WS_MAXIMIZEBOX)
        };
        let (width, height) = outer_size(style, size);
        let class_name = wide(CLASS_NAME);
        let title = wide(title);
        // SAFETY: both strings are NUL-terminated UTF-16 that outlive the call, and the
        // class was registered by this process.
        let handle = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                width,
                height,
                0,
                0,
                module(),
                ptr::null_mut(),
            )
        };
        if handle == 0 {
            return Err(PluginError::Editor("creating the window"));
        }
        CLOSE_REQUESTED.set(false);
        Ok(Self { handle, style })
    }

    pub(super) fn handle(&self) -> Hwnd {
        self.handle
    }

    pub(super) fn style(&self) -> u32 {
        self.style
    }

    pub(super) fn show(&self) {
        // SAFETY: the window is alive and owned by this thread.
        unsafe {
            ShowWindow(self.handle, SW_SHOW);
            SetTimer(self.handle, 1, BACKSTOP_MS, ptr::null());
        }
    }

    /// Waits for one message and handles it; false once the user closes the window.
    pub(super) fn pump(&self) -> bool {
        let mut message = Msg::default();
        // SAFETY: `message` is a writable MSG; messages for any of this thread's windows,
        // including the plugin's child windows, are fetched.
        if unsafe { GetMessageW(&mut message, 0, 0, 0) } <= 0 {
            return false;
        }
        // SAFETY: `message` was just filled in by GetMessageW.
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        !CLOSE_REQUESTED.get()
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: the window is alive and owned by this thread; its timer goes with it.
        unsafe { DestroyWindow(self.handle) };
    }
}

/// Makes `view` the one the window's size changes are passed to.
pub(super) fn set_view(view: Option<ComPtr<IPlugView>>) {
    VIEW.with(|current| {
        if let Ok(mut current) = current.try_borrow_mut() {
            *current = view;
        }
    });
}

pub(super) fn resize_for_view(window: Hwnd, style: u32, size: &ViewRect) {
    let (width, height) = outer_size(style, size);
    PLUGIN_RESIZING.set(true);
    let flags = SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE;
    // SAFETY: plain call; a stale handle only makes it fail.
    unsafe { SetWindowPos(window, 0, 0, 0, width, height, flags) };
    PLUGIN_RESIZING.set(false);
}

/// Wakes the thread that owns `window` so it reads its commands.
pub(super) fn post_wake(window: Hwnd) {
    // SAFETY: PostMessageW may be called from any thread; a stale handle only makes it fail.
    unsafe { PostMessageW(window, WM_APP, 0, 0) };
}

/// The window size whose client area holds the editor at `size`.
fn outer_size(style: u32, size: &ViewRect) -> (i32, i32) {
    let (width, height) = match (size.right - size.left, size.bottom - size.top) {
        (width, height) if width > 0 && height > 0 => (width, height),
        _ => FALLBACK_SIZE,
    };
    let mut rect = Rect {
        right: width,
        bottom: height,
        ..Rect::default()
    };
    // SAFETY: `rect` is a writable RECT.
    if unsafe { AdjustWindowRectEx(&mut rect, style, 0, 0) } == 0 {
        return (width, height);
    }
    (rect.right - rect.left, rect.bottom - rect.top)
}

fn module() -> isize {
    // SAFETY: a null name asks for the executable's own module, which is always loaded.
    unsafe { GetModuleHandleW(ptr::null()) }
}

fn register_class() -> Result<(), PluginError> {
    static REGISTERED: OnceLock<bool> = OnceLock::new();
    let registered = *REGISTERED.get_or_init(|| {
        let class_name = wide(CLASS_NAME);
        let class = WndClassExW {
            size: std::mem::size_of::<WndClassExW>() as u32,
            style: 0,
            window_proc: Some(window_proc),
            class_extra: 0,
            window_extra: 0,
            instance: module(),
            icon: 0,
            // SAFETY: IDC_ARROW is a predefined cursor ID, passed as a pointer by Win32's
            // MAKEINTRESOURCE convention.
            cursor: unsafe { LoadCursorW(0, IDC_ARROW as *const u16) },
            background: COLOR_WINDOW + 1,
            menu_name: ptr::null(),
            class_name: class_name.as_ptr(),
            small_icon: 0,
        };
        // SAFETY: `class` is fully initialized and its strings outlive the call, which
        // copies them.
        unsafe { RegisterClassExW(&class) != 0 }
    });
    if registered {
        Ok(())
    } else {
        Err(PluginError::Editor("registering the window class"))
    }
}

unsafe extern "system" fn window_proc(
    window: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match message {
        // The editor is detached before the window goes, so closing waits for the loop.
        WM_CLOSE => {
            CLOSE_REQUESTED.set(true);
            0
        }
        WM_SIZE if wparam != SIZE_MINIMIZED && !PLUGIN_RESIZING.get() => {
            let view = VIEW.with(|view| view.try_borrow().ok().and_then(|view| view.clone()));
            if let Some(view) = view {
                let mut size = ViewRect {
                    left: 0,
                    top: 0,
                    right: (lparam & 0xFFFF) as i32,
                    bottom: ((lparam >> 16) & 0xFFFF) as i32,
                };
                // SAFETY: the view is attached to this window and `size` is a valid rect.
                unsafe { view.onSize(&mut size) };
            }
            0
        }
        // SAFETY: the arguments are the ones Windows passed for this window.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}
