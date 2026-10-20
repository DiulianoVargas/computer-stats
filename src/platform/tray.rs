//! The Tray Icon, hand-written on Win32 (research R2): a hidden window that owns a
//! `Shell_NotifyIconW` icon and receives its events.
//!
//! The window lives on the UI thread, whose message loop is Slint's (winit), so its `WndProc` is
//! called from inside that loop. It never calls the event handler directly: events are queued with
//! `slint::invoke_from_event_loop` to avoid re-entering Slint.

use std::cell::RefCell;
use std::ffi::c_void;
use std::path::Path;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject, HBITMAP,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Shell::{
    NIF_GUID, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NIM_SETVERSION, NIN_SELECT, NINF_KEY, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ChangeWindowMessageFilterEx, CreateIconIndirect, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyWindow, HICON, ICONINFO, MSGFLT_ALLOW, RegisterClassW, RegisterWindowMessageW,
    SM_CXSMICON, WINDOW_EX_STYLE, WM_APP, WM_CONTEXTMENU, WNDCLASSW, WS_OVERLAPPED,
};
use windows::core::{GUID, PCWSTR, w};

/// What the user did with the Tray Icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    /// Left click, or Enter/Space with keyboard focus.
    Select,
    /// Right click, Shift+F10 or the Menu key; screen coordinates for the Tray Menu.
    ContextMenu { x: i32, y: i32 },
}

pub const WINDOW_CLASS: PCWSTR = w!("ComputerStats.Tray");
const CALLBACK_MESSAGE: u32 = WM_APP + 1;
/// `NIN_KEYSELECT` (not exported by the `windows` crate).
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;

type Handler = Box<dyn Fn(TrayEvent)>;

struct State {
    hwnd: HWND,
    guid: GUID,
    icon: Option<HICON>,
    tooltip: String,
    taskbar_created: u32,
    handler: std::rc::Rc<Handler>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// The app's single Tray Icon. Removed on drop. Not `Send`: it belongs to the UI thread.
pub struct TrayIcon {
    _not_send: std::marker::PhantomData<*const ()>,
}

impl TrayIcon {
    /// Creates the hidden window and adds the icon (with no image yet; call `set_icon`).
    pub fn create(on_event: impl Fn(TrayEvent) + 'static) -> Result<TrayIcon, String> {
        // SAFETY: plain Win32 calls with valid, NUL-terminated arguments.
        let hwnd = unsafe {
            let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(wnd_proc),
                hInstance: instance.into(),
                lpszClassName: WINDOW_CLASS,
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                return Err("RegisterClassW failed".into());
            }
            // A regular hidden top-level window (not message-only) so it receives the broadcast
            // `TaskbarCreated` message.
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                WINDOW_CLASS,
                w!("Computer Stats"),
                WS_OVERLAPPED,
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

        // SAFETY: the string is a valid NUL-terminated literal.
        let taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
        // We run elevated; Explorer does not. Without this, UIPI drops its messages (FR-024).
        for msg in [taskbar_created, CALLBACK_MESSAGE, WM_CONTEXTMENU] {
            // SAFETY: `hwnd` is our live window.
            let _ = unsafe { ChangeWindowMessageFilterEx(hwnd, msg, MSGFLT_ALLOW, None) };
        }

        let guid = std::env::current_exe()
            .map(|p| icon_guid(&p))
            .unwrap_or(BASE_GUID);
        STATE.with_borrow_mut(|s| {
            *s = Some(State {
                hwnd,
                guid,
                icon: None,
                tooltip: crate::snapshot::APP_NAME.to_string(),
                taskbar_created,
                handler: std::rc::Rc::new(Box::new(on_event)),
            })
        });

        if !add_icon() {
            return Err("Shell_NotifyIconW(NIM_ADD) failed".into());
        }
        Ok(TrayIcon {
            _not_send: std::marker::PhantomData,
        })
    }

    /// The small-icon size in physical pixels at the taskbar's DPI.
    pub fn icon_size(&self) -> u32 {
        let Some(hwnd) = STATE.with_borrow(|s| s.as_ref().map(|s| s.hwnd)) else {
            return 16;
        };
        // SAFETY: `hwnd` is our live window.
        let size = unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForWindow(hwnd)) };
        if size > 0 { size as u32 } else { 16 }
    }

    /// Replaces the icon image with premultiplied ARGB `pixels` (`size × size`, top row first).
    pub fn set_icon(&self, pixels: &[u32], size: u32) {
        let Some(icon) = make_icon(pixels, size) else {
            return;
        };
        let old = STATE.with_borrow_mut(|s| s.as_mut().and_then(|s| s.icon.replace(icon)));
        modify();
        if let Some(old) = old {
            // SAFETY: `old` was created by us and is no longer used by the shell.
            let _ = unsafe { DestroyIcon(old) };
        }
    }

    /// Updates the tooltip; does nothing when the text is unchanged.
    pub fn set_tooltip(&self, text: &str) {
        let changed = STATE.with_borrow_mut(|s| match s {
            Some(s) if s.tooltip != text => {
                s.tooltip = text.to_string();
                true
            }
            _ => false,
        });
        if changed {
            modify();
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        let Some(state) = STATE.with_borrow_mut(|s| s.take()) else {
            return;
        };
        let mut nid = base_data(&state);
        nid.uFlags = NIF_GUID;
        // SAFETY: `nid` identifies our icon; the window and icon are ours to destroy.
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            if let Some(icon) = state.icon {
                let _ = DestroyIcon(icon);
            }
            let _ = DestroyWindow(state.hwnd);
        }
    }
}

// --- NOTIFYICONDATA --------------------------------------------------------------------------

fn base_data(state: &State) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: state.hwnd,
        guidItem: state.guid,
        ..Default::default()
    }
}

fn full_data(state: &State) -> NOTIFYICONDATAW {
    let mut nid = base_data(state);
    nid.uFlags = NIF_GUID | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
    nid.uCallbackMessage = CALLBACK_MESSAGE;
    if let Some(icon) = state.icon {
        nid.uFlags |= NIF_ICON;
        nid.hIcon = icon;
    }
    let tip: Vec<u16> = state.tooltip.encode_utf16().collect();
    let n = tip.len().min(nid.szTip.len() - 1);
    nid.szTip[..n].copy_from_slice(&tip[..n]);
    nid
}

/// Adds the icon (startup and after Explorer restarts). Returns whether it succeeded.
fn add_icon() -> bool {
    STATE.with_borrow(|s| {
        let Some(state) = s else { return false };
        let mut nid = full_data(state);
        // SAFETY: `nid` is fully initialized and identifies our window.
        unsafe {
            // A stale icon with our GUID (e.g. after a crash) makes NIM_ADD fail; remove it first.
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
                return false;
            }
            nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            let _ = Shell_NotifyIconW(NIM_SETVERSION, &nid);
        }
        true
    })
}

fn modify() {
    STATE.with_borrow(|s| {
        if let Some(state) = s {
            let nid = full_data(state);
            // SAFETY: `nid` is fully initialized and identifies our icon.
            let _ = unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
        }
    });
}

// --- Icon GUID -------------------------------------------------------------------------------

const BASE_GUID: GUID = GUID::from_u128(0x5c1f_7a2e_9b3d_4e61_8a07_c4d2_e6f1_3b90);

/// The shell ties an icon GUID to the executable's path, so a fixed GUID fails when the exe moves
/// (e.g. debug vs release builds). Mixing in a hash of the path keeps it stable per location.
pub fn icon_guid(exe: &Path) -> GUID {
    // FNV-1a over the lowercase path: stable across runs and Rust versions.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in exe.to_string_lossy().to_lowercase().bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    GUID::from_u128(BASE_GUID.to_u128() ^ hash as u128)
}

// --- HICON from pixels -----------------------------------------------------------------------

fn make_icon(pixels: &[u32], size: u32) -> Option<HICON> {
    let n = (size * size) as usize;
    if size == 0 || pixels.len() != n {
        return None;
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size as i32,
            biHeight: -(size as i32), // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    // SAFETY: `info` describes a 32-bpp top-down DIB of `n` pixels; `bits` receives its buffer,
    // which we fill with exactly `n` u32 values before handing it to CreateIconIndirect.
    unsafe {
        let color: HBITMAP =
            CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if bits.is_null() {
            let _ = DeleteObject(color.into());
            return None;
        }
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u32, n);
        // The alpha channel defines transparency; the AND mask is all zeros.
        let mask = CreateBitmap(size as i32, size as i32, 1, 1, None);
        let icon_info = ICONINFO {
            fIcon: true.into(),
            hbmMask: mask,
            hbmColor: color,
            ..Default::default()
        };
        let icon = CreateIconIndirect(&icon_info).ok();
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon
    }
}

// --- Window procedure ------------------------------------------------------------------------

fn dispatch(event: TrayEvent) {
    // Queue the event so the handler runs outside this WndProc (no Slint re-entrancy).
    let _ = slint::invoke_from_event_loop(move || {
        let handler = STATE.with_borrow(|s| s.as_ref().map(|s| std::rc::Rc::clone(&s.handler)));
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
    if msg == CALLBACK_MESSAGE {
        // NOTIFYICON_VERSION_4: LOWORD(lParam) is the event, wParam holds the anchor point.
        let event = (lparam.0 & 0xFFFF) as u32;
        let x = (wparam.0 & 0xFFFF) as u16 as i16 as i32;
        let y = ((wparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
        match event {
            NIN_SELECT | NIN_KEYSELECT => dispatch(TrayEvent::Select),
            WM_CONTEXTMENU => dispatch(TrayEvent::ContextMenu { x, y }),
            _ => {}
        }
        return LRESULT(0);
    }
    let taskbar_created = STATE.with_borrow(|s| s.as_ref().map(|s| s.taskbar_created));
    if taskbar_created == Some(msg) && msg != 0 {
        add_icon();
        return LRESULT(0);
    }
    // SAFETY: forwarding the unchanged message to the default procedure.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_is_stable_per_path_and_case_insensitive() {
        let a = icon_guid(Path::new(r"C:\Apps\computer-stats.exe"));
        let b = icon_guid(Path::new(r"c:\apps\COMPUTER-STATS.EXE"));
        let c = icon_guid(Path::new(r"C:\Other\computer-stats.exe"));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
