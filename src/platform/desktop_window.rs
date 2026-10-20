//! The Desktop Widget's window (005 research R1–R4): a layered Win32 popup that shows an image
//! with per-pixel alpha over the wallpaper, stays behind every other window, never takes focus,
//! and is dragged natively.
//!
//! Like the Tray Icon's window, it lives on the UI thread inside Slint's (winit) message loop, and
//! its `WndProc` never calls the event handler directly: events are queued with
//! `slint::invoke_from_event_loop` to avoid re-entering Slint.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::rc::Rc;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC,
    HGDIOBJ, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, EVENT_SYSTEM_FOREGROUND, GWL_EXSTYLE,
    GetClassNameW, GetWindowLongPtrW, GetWindowRect, HTCAPTION, HWND_BOTTOM, HWND_NOTOPMOST,
    HWND_TOPMOST, MA_NOACTIVATE, RegisterClassW, SPI_SETWORKAREA, SW_HIDE, SW_SHOWNOACTIVATE,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos, ShowWindow, ULW_ALPHA,
    UpdateLayeredWindow, WINDOWPOS, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_CLOSE,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_EXITSIZEMOVE, WM_MOUSEACTIVATE, WM_NCHITTEST,
    WM_NCLBUTTONDBLCLK, WM_NCRBUTTONDOWN, WM_NCRBUTTONUP, WM_SETTINGCHANGE, WM_WINDOWPOSCHANGING,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::domain::anchor::{Point, Rect};

/// What happened to the Desktop Widget's window, for the UI thread.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WidgetEvent {
    /// The user finished dragging it; its new top-left.
    Moved(Point),
    /// It is now on a monitor with this scale (1.0 = 96 DPI).
    DpiChanged(f32),
    /// Monitors, resolution or work areas changed.
    DisplayChanged,
}

const WINDOW_CLASS: PCWSTR = w!("ComputerStats.DesktopWidget");

type Handler = Rc<Box<dyn Fn(WidgetEvent)>>;

thread_local! {
    /// The live window, for the WinEvent hook (0 when none).
    static HWND_VALUE: Cell<isize> = const { Cell::new(0) };
    /// Raised over the desktop during Show desktop (research R4): the bottom guard is off.
    static RAISED: Cell<bool> = const { Cell::new(false) };
    static HANDLER: RefCell<Option<Handler>> = const { RefCell::new(None) };
}

/// The image buffer `UpdateLayeredWindow` reads from: a 32-bpp top-down DIB selected into a
/// memory DC, re-created only when the size changes.
struct Dib {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
    width: u32,
    height: u32,
}

/// The Desktop Widget's window. One at a time; destroyed on drop. Not `Send`: it belongs to the UI
/// thread.
pub struct DesktopWindow {
    hwnd: HWND,
    hook: HWINEVENTHOOK,
    dib: Option<Dib>,
}

impl DesktopWindow {
    /// Creates the hidden window and the Show desktop hook. Events go to `on_event`.
    pub fn create(on_event: impl Fn(WidgetEvent) + 'static) -> Result<DesktopWindow, String> {
        // SAFETY: plain Win32 calls with valid, NUL-terminated arguments.
        let hwnd = unsafe {
            let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(wnd_proc),
                hInstance: instance.into(),
                lpszClassName: WINDOW_CLASS,
                ..Default::default()
            };
            // Fails harmlessly when already registered by an earlier Desktop Widget.
            RegisterClassW(&class);
            // No owner: an owned window always sits above its owner, which would fight
            // HWND_BOTTOM (research R3). TOOLWINDOW keeps it out of the taskbar and Alt+Tab.
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                WINDOW_CLASS,
                w!("Computer Stats widget"),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|e| e.to_string())?
        };
        HWND_VALUE.set(hwnd.0 as isize);
        RAISED.set(false);
        HANDLER.set(Some(Rc::new(Box::new(on_event))));

        // Out of context: nothing is injected into other processes (Constitution IV).
        // SAFETY: `foreground_hook` matches WINEVENTPROC and stays valid for the program's life.
        let hook = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(foreground_hook),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        Ok(DesktopWindow {
            hwnd,
            hook,
            dib: None,
        })
    }

    /// Shows `rgba` (straight-alpha RGBA, `width × height`, top row first), moving the window's
    /// top-left to `move_to` if given; `None` keeps it where it is (e.g. while being dragged).
    /// Does nothing if the buffer does not match the size.
    pub fn present(&mut self, rgba: &[u8], width: u32, height: u32, move_to: Option<Point>) {
        let n = width as usize * height as usize;
        if n == 0 || rgba.len() != n * 4 {
            return;
        }
        let hwnd = self.hwnd;
        let Some(dib) = self.dib_for(width, height) else {
            return;
        };
        // SAFETY: `dib.bits` points to the DIB's `width × height` 32-bpp pixels, which nothing
        // else accesses while this slice lives.
        let target = unsafe { std::slice::from_raw_parts_mut(dib.bits, n * 4) };
        // Layered windows take premultiplied BGRA.
        for (out, px) in target
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(rgba.as_chunks::<4>().0)
        {
            let a = px[3] as u32;
            let premultiply = |c: u8| ((c as u32 * a + 127) / 255) as u8;
            out[0] = premultiply(px[2]);
            out[1] = premultiply(px[1]);
            out[2] = premultiply(px[0]);
            out[3] = px[3];
        }
        let position = move_to.map(|p| POINT { x: p.x, y: p.y });
        let size = SIZE {
            cx: width as i32,
            cy: height as i32,
        };
        let source = POINT::default();
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        // SAFETY: all pointers refer to locals that outlive the call; `dib.dc` holds the bitmap.
        let _ = unsafe {
            UpdateLayeredWindow(
                hwnd,
                None,
                position.as_ref().map(|p| p as *const POINT),
                Some(&size),
                Some(dib.dc),
                Some(&source),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
        };
    }

    /// Shows the window behind every other window, without activating it (FR-004, FR-005).
    pub fn show(&self) {
        // SAFETY: `self.hwnd` is our live window.
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_BOTTOM),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }

    pub fn hide(&self) {
        // SAFETY: `self.hwnd` is our live window.
        let _ = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
    }

    /// The window's screen rectangle in physical pixels.
    pub fn rect(&self) -> Option<Rect> {
        let mut r = RECT::default();
        // SAFETY: `r` is a valid out-pointer; `self.hwnd` is our live window.
        unsafe { GetWindowRect(self.hwnd, &mut r) }
            .is_ok()
            .then(|| super::shell::from_win(r))
    }

    fn dib_for(&mut self, width: u32, height: u32) -> Option<&Dib> {
        if self
            .dib
            .as_ref()
            .is_some_and(|d| d.width != width || d.height != height)
        {
            free_dib(self.dib.take());
        }
        if self.dib.is_none() {
            self.dib = create_dib(width, height);
        }
        self.dib.as_ref()
    }
}

impl Drop for DesktopWindow {
    fn drop(&mut self) {
        HWND_VALUE.set(0);
        RAISED.set(false);
        HANDLER.set(None);
        // SAFETY: the hook and window are ours; nothing uses them after this.
        unsafe {
            if !self.hook.is_invalid() {
                let _ = UnhookWinEvent(self.hook);
            }
            let _ = DestroyWindow(self.hwnd);
        }
        free_dib(self.dib.take());
    }
}

fn create_dib(width: u32, height: u32) -> Option<Dib> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32), // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    // SAFETY: `info` describes a 32-bpp top-down DIB; every object created here is either
    // returned in the Dib (and freed by `free_dib`) or freed before returning `None`.
    unsafe {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return None;
        }
        let bitmap = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(b) if !bits.is_null() => b,
            Ok(b) => {
                let _ = DeleteObject(b.into());
                let _ = DeleteDC(dc);
                return None;
            }
            Err(_) => {
                let _ = DeleteDC(dc);
                return None;
            }
        };
        let old = SelectObject(dc, bitmap.into());
        Some(Dib {
            dc,
            bitmap,
            old,
            bits: bits as *mut u8,
            width,
            height,
        })
    }
}

fn free_dib(dib: Option<Dib>) {
    if let Some(dib) = dib {
        // SAFETY: the DC and bitmap were created by `create_dib` and are no longer used.
        unsafe {
            SelectObject(dib.dc, dib.old);
            let _ = DeleteObject(dib.bitmap.into());
            let _ = DeleteDC(dib.dc);
        }
    }
}

// --- Show desktop (research R4) --------------------------------------------------------------

/// Whether `hwnd` is the desktop brought to the front by Show desktop: a shell desktop window
/// (`WorkerW` / `Progman`) that is topmost. A plain click on the desktop also makes it foreground,
/// but not topmost, and must not raise the widget over other windows.
fn is_shown_desktop(hwnd: HWND) -> bool {
    let mut name = [0u16; 16];
    // SAFETY: `name` is a valid buffer of the given length.
    let len = unsafe { GetClassNameW(hwnd, &mut name) };
    let class = String::from_utf16_lossy(&name[..len.max(0) as usize]);
    if class != "WorkerW" && class != "Progman" {
        return false;
    }
    // SAFETY: reading a style of any window is allowed.
    let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    ex_style & WS_EX_TOPMOST.0 != 0
}

unsafe extern "system" fn foreground_hook(
    _hook: HWINEVENTHOOK,
    _event: u32,
    foreground: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    let hwnd = HWND(HWND_VALUE.get() as *mut c_void);
    if hwnd.is_invalid() {
        return;
    }
    let raise = is_shown_desktop(foreground);
    if raise == RAISED.get() {
        return;
    }
    RAISED.set(raise);
    let flags = SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE;
    // SAFETY: `hwnd` is our live window (cleared before it is destroyed).
    unsafe {
        if raise {
            // Above the topmost desktop, until another window comes to the front.
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, flags);
        } else {
            let _ = SetWindowPos(hwnd, Some(HWND_NOTOPMOST), 0, 0, 0, 0, flags);
            let _ = SetWindowPos(hwnd, Some(HWND_BOTTOM), 0, 0, 0, 0, flags);
        }
    }
}

// --- Window procedure ------------------------------------------------------------------------

fn dispatch(event: WidgetEvent) {
    let _ = slint::invoke_from_event_loop(move || {
        let handler = HANDLER.with_borrow(|h| h.clone());
        if let Some(handler) = handler {
            handler(event);
        }
    });
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // Drag from anywhere (FR-015); Windows moves the window.
        WM_NCHITTEST => return LRESULT(HTCAPTION as isize),
        // No maximize on double-click, no system menu (Move/Close) on right-click, no close.
        WM_NCLBUTTONDBLCLK | WM_NCRBUTTONDOWN | WM_NCRBUTTONUP | WM_CLOSE => return LRESULT(0),
        // Clicks never take focus from the user's window (FR-005).
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
        WM_WINDOWPOSCHANGING => {
            if !RAISED.get() && lparam.0 != 0 {
                // SAFETY: for this message `lparam` points to a WINDOWPOS we may modify.
                let pos = unsafe { &mut *(lparam.0 as *mut WINDOWPOS) };
                // Whatever brought it forward, it stays behind every window (FR-004, research R3).
                pos.hwndInsertAfter = HWND_BOTTOM;
                pos.flags &= !SWP_NOZORDER;
            }
            return LRESULT(0);
        }
        WM_EXITSIZEMOVE => {
            let mut r = RECT::default();
            // SAFETY: `r` is a valid out-pointer.
            if unsafe { GetWindowRect(hwnd, &mut r) }.is_ok() {
                dispatch(WidgetEvent::Moved(Point {
                    x: r.left,
                    y: r.top,
                }));
            }
            return LRESULT(0);
        }
        WM_DPICHANGED => {
            // The app re-renders at the new scale and keeps the top-left; the suggested rect is
            // not applied.
            let dpi = (wparam.0 >> 16) & 0xFFFF;
            if dpi > 0 {
                dispatch(WidgetEvent::DpiChanged(dpi as f32 / 96.0));
            }
            return LRESULT(0);
        }
        WM_DISPLAYCHANGE => dispatch(WidgetEvent::DisplayChanged),
        WM_SETTINGCHANGE if wparam.0 == SPI_SETWORKAREA.0 as usize => {
            dispatch(WidgetEvent::DisplayChanged)
        }
        _ => {}
    }
    // SAFETY: forwarding the unchanged message to the default procedure.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
