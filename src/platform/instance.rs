//! Single instance (FR-023; research R6): a named mutex, and a message asking the running
//! instance to open its Panel.

use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, RegisterWindowMessageW};
use windows::core::w;

use super::tray::WINDOW_CLASS;

/// Held for the process lifetime; Windows releases the mutex when the process exits.
pub struct InstanceGuard {
    _mutex: HANDLE,
}

pub enum Instance {
    First(InstanceGuard),
    /// Another instance is running and was asked to open its Panel.
    AlreadyRunning,
}

/// The registered message the running instance treats as "open the Panel".
pub fn open_panel_message() -> u32 {
    // SAFETY: literal, NUL-terminated name.
    unsafe { RegisterWindowMessageW(w!("ComputerStats.OpenPanel")) }
}

pub fn acquire() -> Instance {
    // SAFETY: literal, NUL-terminated name; GetLastError is read right after the call.
    let (mutex, error) = unsafe {
        let mutex = CreateMutexW(None, true, w!("Local\\ComputerStats.SingleInstance"));
        (mutex, GetLastError())
    };
    // An elevated instance's mutex may deny a non-elevated process: that is still "running".
    let running = error == ERROR_ALREADY_EXISTS || error == ERROR_ACCESS_DENIED;
    match mutex {
        Ok(handle) if !running => Instance::First(InstanceGuard { _mutex: handle }),
        _ => {
            // SAFETY: class name is a literal; posting to a found window has no preconditions.
            unsafe {
                if let Ok(hwnd) = FindWindowW(WINDOW_CLASS, None) {
                    let _ = PostMessageW(
                        Some(hwnd),
                        open_panel_message(),
                        Default::default(),
                        Default::default(),
                    );
                }
            }
            Instance::AlreadyRunning
        }
    }
}
