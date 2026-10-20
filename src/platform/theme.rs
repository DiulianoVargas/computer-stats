//! Windows theme (research R3).

use std::ffi::c_void;

use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::core::{PCWSTR, w};

/// Whether the taskbar ("system") uses the light theme. Missing value → dark (Windows default).
pub fn taskbar_uses_light_theme() -> bool {
    personalize_flag(w!("SystemUsesLightTheme")).unwrap_or(false)
}

/// Whether apps use the light theme, for the Desktop Widget (005 research R7). Missing value →
/// light (Windows default for apps).
pub fn apps_use_light_theme() -> bool {
    personalize_flag(w!("AppsUseLightTheme")).unwrap_or(true)
}

fn personalize_flag(name: PCWSTR) -> Option<bool> {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: `value` and `size` describe a valid writable DWORD buffer; `name` is a literal.
    let err = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            name,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut c_void),
            Some(&mut size),
        )
    };
    err.is_ok().then_some(value != 0)
}
