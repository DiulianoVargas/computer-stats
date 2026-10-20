//! The Desktop Widget's remembered state: DWORDs under `HKCU\Software\Computer Stats` (005
//! research R5). Every failure reads as "off" or is ignored.

use std::ffi::c_void;

use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_DWORD, RegGetValueW, RegSetKeyValueW,
};
use windows::core::{PCWSTR, w};

use crate::domain::anchor::Point;
use crate::domain::widget_place::WidgetSettings;

const KEY: PCWSTR = w!(r"Software\Computer Stats");
const SHOWN: PCWSTR = w!("DesktopWidget");
const X: PCWSTR = w!("DesktopWidgetX");
const Y: PCWSTR = w!("DesktopWidgetY");

pub fn load() -> WidgetSettings {
    let Some(shown) = read(SHOWN) else {
        return WidgetSettings::default();
    };
    let position = read(X).zip(read(Y)).map(|(x, y)| Point {
        x: x as i32,
        y: y as i32,
    });
    WidgetSettings {
        shown: shown != 0,
        position,
    }
}

pub fn save_shown(shown: bool) {
    write(SHOWN, shown as u32);
}

pub fn save_position(p: Point) {
    // Two's complement: monitors left of or above the primary have negative coordinates.
    write(X, p.x as u32);
    write(Y, p.y as u32);
}

fn read(name: PCWSTR) -> Option<u32> {
    let mut value = 0u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: `value` and `size` describe a valid writable DWORD buffer; strings are literals.
    let err = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            KEY,
            name,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut c_void),
            Some(&mut size),
        )
    };
    err.is_ok().then_some(value)
}

fn write(name: PCWSTR, value: u32) {
    // SAFETY: `value` is a live DWORD of the given size; the key is created if missing.
    let _ = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            KEY,
            name,
            REG_DWORD.0,
            Some(&value as *const u32 as *const c_void),
            size_of::<u32>() as u32,
        )
    };
}
