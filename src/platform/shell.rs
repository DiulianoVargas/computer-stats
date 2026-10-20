//! Taskbar and monitor geometry for placing the Panel (research R5), and the screen capture.

use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, MONITORINFO,
    MonitorFromPoint, MonitorFromRect,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Shell::{
    ABE_LEFT, ABE_RIGHT, ABE_TOP, ABM_GETTASKBARPOS, APPBARDATA, SHAppBarMessage, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SW_SHOWNORMAL};
use windows::core::w;

use crate::domain::anchor::{Edge, Rect};

/// The edge the taskbar is docked to; bottom when unknown.
pub fn taskbar_edge() -> Edge {
    let mut data = APPBARDATA {
        cbSize: size_of::<APPBARDATA>() as u32,
        ..Default::default()
    };
    // SAFETY: `data` is a valid APPBARDATA with `cbSize` set.
    if unsafe { SHAppBarMessage(ABM_GETTASKBARPOS, &mut data) } == 0 {
        return Edge::Bottom;
    }
    match data.uEdge {
        ABE_LEFT => Edge::Left,
        ABE_TOP => Edge::Top,
        ABE_RIGHT => Edge::Right,
        _ => Edge::Bottom,
    }
}

/// The work area (screen minus taskbar) of the monitor nearest to `rect`.
pub fn work_area_for(rect: Rect) -> Option<Rect> {
    let monitor = {
        let r = to_win(rect);
        // SAFETY: `r` is a valid RECT; NEAREST always returns a monitor.
        unsafe { MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST) }
    };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is a valid MONITORINFO with `cbSize` set.
    unsafe { GetMonitorInfoW(monitor, &mut info) }
        .as_bool()
        .then(|| from_win(info.rcWork))
}

/// The primary monitor's work area: where the Desktop Widget first appears (005 research R6).
pub fn primary_work_area() -> Option<Rect> {
    // SAFETY: PRIMARY always returns a monitor.
    let monitor = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is a valid MONITORINFO with `cbSize` set.
    unsafe { GetMonitorInfoW(monitor, &mut info) }
        .as_bool()
        .then(|| from_win(info.rcWork))
}

/// Display scale (1.0 = 96 DPI) of the monitor nearest to `rect`.
pub fn scale_for(rect: Rect) -> f32 {
    let r = to_win(rect);
    let (mut x, mut y) = (96u32, 96u32);
    // SAFETY: `r` is a valid RECT; out-pointers are valid u32s.
    let ok = unsafe {
        let monitor = MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y)
    }
    .is_ok();
    if ok && x > 0 { x as f32 / 96.0 } else { 1.0 }
}

/// The cursor as a 1×1 rect: the fallback anchor when the icon rect is unknown (overflow flyout).
pub fn cursor_rect() -> Rect {
    let mut p = POINT::default();
    // SAFETY: `p` is a valid out-pointer.
    let _ = unsafe { GetCursorPos(&mut p) };
    Rect {
        left: p.x,
        top: p.y,
        right: p.x + 1,
        bottom: p.y + 1,
    }
}

pub(super) fn from_win(r: RECT) -> Rect {
    Rect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    }
}

fn to_win(r: Rect) -> RECT {
    RECT {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    }
}

/// Opens the Windows screen capture (Snipping Tool's overlay). Print Screen does not reach it on
/// its own while the elevated Panel has focus: its keyboard hook runs unelevated. False if it
/// could not be started.
pub fn open_screen_capture() -> bool {
    // SAFETY: constant, NUL-terminated wide strings; no owner window.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            w!("ms-screenclip:"),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    // Values above 32 mean success.
    result.0 as usize > 32
}
