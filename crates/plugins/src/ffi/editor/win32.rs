//! The few user32 and kernel32 functions an editor window needs, declared by hand.

use std::ffi::c_void;

pub(super) type Hwnd = isize;
pub(super) type WindowProc = unsafe extern "system" fn(Hwnd, u32, usize, isize) -> isize;

pub(super) const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
pub(super) const WS_THICKFRAME: u32 = 0x0004_0000;
pub(super) const WS_MAXIMIZEBOX: u32 = 0x0001_0000;
pub(super) const CW_USEDEFAULT: i32 = i32::MIN;
pub(super) const SW_SHOW: i32 = 5;
pub(super) const WM_SIZE: u32 = 0x0005;
pub(super) const WM_CLOSE: u32 = 0x0010;
pub(super) const WM_APP: u32 = 0x8000;
pub(super) const SIZE_MINIMIZED: usize = 1;
pub(super) const SWP_NOMOVE: u32 = 0x0002;
pub(super) const SWP_NOZORDER: u32 = 0x0004;
pub(super) const SWP_NOACTIVATE: u32 = 0x0010;
pub(super) const IDC_ARROW: usize = 32512;
pub(super) const COLOR_WINDOW: isize = 5;

#[repr(C)]
pub(super) struct WndClassExW {
    pub(super) size: u32,
    pub(super) style: u32,
    pub(super) window_proc: Option<WindowProc>,
    pub(super) class_extra: i32,
    pub(super) window_extra: i32,
    pub(super) instance: isize,
    pub(super) icon: isize,
    pub(super) cursor: isize,
    pub(super) background: isize,
    pub(super) menu_name: *const u16,
    pub(super) class_name: *const u16,
    pub(super) small_icon: isize,
}

#[repr(C)]
#[derive(Default)]
pub(super) struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Default)]
pub(super) struct Msg {
    window: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: Point,
}

#[repr(C)]
#[derive(Default)]
pub(super) struct Rect {
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) right: i32,
    pub(super) bottom: i32,
}

#[link(name = "user32")]
extern "system" {
    pub(super) fn RegisterClassExW(class: *const WndClassExW) -> u16;
    #[allow(clippy::too_many_arguments)]
    pub(super) fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Hwnd,
        menu: isize,
        instance: isize,
        param: *mut c_void,
    ) -> Hwnd;
    pub(super) fn DefWindowProcW(window: Hwnd, message: u32, wparam: usize, lparam: isize)
        -> isize;
    pub(super) fn DestroyWindow(window: Hwnd) -> i32;
    pub(super) fn ShowWindow(window: Hwnd, command: i32) -> i32;
    pub(super) fn GetMessageW(message: *mut Msg, window: Hwnd, min: u32, max: u32) -> i32;
    pub(super) fn TranslateMessage(message: *const Msg) -> i32;
    pub(super) fn DispatchMessageW(message: *const Msg) -> isize;
    pub(super) fn PostMessageW(window: Hwnd, message: u32, wparam: usize, lparam: isize) -> i32;
    pub(super) fn SetWindowPos(
        window: Hwnd,
        insert_after: Hwnd,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    pub(super) fn AdjustWindowRectEx(rect: *mut Rect, style: u32, menu: i32, ex_style: u32) -> i32;
    pub(super) fn LoadCursorW(instance: isize, name: *const u16) -> isize;
    pub(super) fn SetTimer(
        window: Hwnd,
        id: usize,
        milliseconds: u32,
        func: *const c_void,
    ) -> usize;
}

#[link(name = "kernel32")]
extern "system" {
    pub(super) fn GetModuleHandleW(name: *const u16) -> isize;
}

pub(super) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
