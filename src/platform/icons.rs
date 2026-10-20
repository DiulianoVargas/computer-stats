//! An App's display name and icon from its program file (003 research R5).

use std::ffi::c_void;

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits,
    GetObjectW, HBITMAP, ReleaseDC,
};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::UI::Shell::ExtractIconExW;
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};
use windows::core::PCWSTR;

use crate::domain::apps::IconPixels;

/// The program's `FileDescription` (e.g. "Google Chrome"), if it has a non-empty one.
pub fn file_description(path: &str) -> Option<String> {
    let file = wide(path);
    // SAFETY: `file` is NUL-terminated.
    let size = unsafe { GetFileVersionInfoSizeW(PCWSTR(file.as_ptr()), None) };
    if size == 0 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    // SAFETY: `data` has exactly `size` bytes.
    unsafe {
        GetFileVersionInfoW(
            PCWSTR(file.as_ptr()),
            None,
            size,
            data.as_mut_ptr() as *mut c_void,
        )
    }
    .ok()?;

    // Language/code page pairs; fall back to US English, Unicode.
    let mut languages = vec![(0x0409u16, 0x04B0u16)];
    if let Some(bytes) = query(&data, r"\VarFileInfo\Translation") {
        languages = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| {
                (
                    u16::from_le_bytes([c[0], c[1]]),
                    u16::from_le_bytes([c[2], c[3]]),
                )
            })
            .chain(languages)
            .collect();
    }
    languages.into_iter().find_map(|(lang, cp)| {
        let bytes = query(
            &data,
            &format!(r"\StringFileInfo\{lang:04x}{cp:04x}\FileDescription"),
        )?;
        let units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&u| u != 0)
            .collect();
        let text = String::from_utf16_lossy(&units).trim().to_string();
        (!text.is_empty()).then_some(text)
    })
}

/// A copy of the bytes `VerQueryValueW` points to for `sub_block`. String values report their
/// length in characters, so a copy of up to twice that is taken and trimmed at the NUL.
fn query(data: &[u8], sub_block: &str) -> Option<Vec<u8>> {
    let sub = wide(sub_block);
    let mut ptr: *mut c_void = std::ptr::null_mut();
    let mut len = 0u32;
    // SAFETY: `data` is a version-info block; `ptr`/`len` are valid out-pointers.
    let ok = unsafe {
        VerQueryValueW(
            data.as_ptr() as *const c_void,
            PCWSTR(sub.as_ptr()),
            &mut ptr,
            &mut len,
        )
    }
    .as_bool();
    if !ok || ptr.is_null() || len == 0 {
        return None;
    }
    let start = ptr as usize;
    let lo = data.as_ptr() as usize;
    let hi = lo + data.len();
    if start < lo || start >= hi {
        return None;
    }
    let n = ((len as usize) * 2).min(hi - start);
    // SAFETY: `[start, start + n)` lies inside `data`.
    Some(unsafe { std::slice::from_raw_parts(ptr as *const u8, n) }.to_vec())
}

/// The program's first small icon as straight RGBA.
pub fn small_icon(path: &str) -> Option<IconPixels> {
    let file = wide(path);
    let mut icon = HICON::default();
    // SAFETY: `file` is NUL-terminated; one small icon is written to `icon`.
    let count = unsafe { ExtractIconExW(PCWSTR(file.as_ptr()), 0, None, Some(&mut icon), 1) };
    if count == 0 || icon.is_invalid() {
        return None;
    }
    let pixels = icon_pixels(icon);
    // SAFETY: `icon` was created by ExtractIconExW and is ours.
    let _ = unsafe { DestroyIcon(icon) };
    pixels
}

fn icon_pixels(icon: HICON) -> Option<IconPixels> {
    let mut info = ICONINFO::default();
    // SAFETY: `info` is a valid out-pointer; the bitmaps it returns are deleted below.
    unsafe { GetIconInfo(icon, &mut info) }.ok()?;
    let result = (|| {
        let mut bitmap = BITMAP::default();
        // SAFETY: `bitmap` is a valid BITMAP of the size passed.
        let got = unsafe {
            GetObjectW(
                info.hbmColor.into(),
                size_of::<BITMAP>() as i32,
                Some(&mut bitmap as *mut _ as *mut c_void),
            )
        };
        if got == 0 || bitmap.bmWidth <= 0 || bitmap.bmWidth != bitmap.bmHeight {
            return None;
        }
        let size = bitmap.bmWidth as u32;
        let color = dib(info.hbmColor, size)?;
        let mask = dib(info.hbmMask, size);
        let has_alpha = color.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
        let mut rgba = Vec::with_capacity(color.len());
        for (i, p) in color.as_chunks::<4>().0.iter().enumerate() {
            // Icons without alpha use the AND mask: white (non-zero) means transparent.
            let alpha = if has_alpha {
                p[3]
            } else if mask.as_ref().is_some_and(|m| m[i * 4] != 0) {
                0
            } else {
                255
            };
            rgba.extend_from_slice(&[p[2], p[1], p[0], alpha]);
        }
        Some(IconPixels { size, rgba })
    })();
    // SAFETY: the bitmaps came from GetIconInfo and are ours to delete.
    unsafe {
        let _ = DeleteObject(info.hbmColor.into());
        let _ = DeleteObject(info.hbmMask.into());
    }
    result
}

/// A bitmap's pixels as 32-bpp top-down BGRA.
fn dib(bitmap: HBITMAP, size: u32) -> Option<Vec<u8>> {
    if bitmap.is_invalid() {
        return None;
    }
    let mut out = vec![0u8; (size * size * 4) as usize];
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size as i32,
            biHeight: -(size as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: `out` holds `size` rows of `size` 32-bpp pixels, as `info` describes; the screen DC
    // is released right after.
    let lines = unsafe {
        let dc = GetDC(None);
        let lines = GetDIBits(
            dc,
            bitmap,
            0,
            size,
            Some(out.as_mut_ptr() as *mut c_void),
            &mut info,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, dc);
        lines
    };
    (lines == size as i32).then_some(out)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_system_program() {
        let windir = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let notepad = format!(r"{windir}\System32\notepad.exe");
        assert!(file_description(&notepad).is_some_and(|d| !d.is_empty()));
        let icon = small_icon(&notepad);
        assert!(
            icon.is_some_and(|i| i.size >= 16 && i.rgba.len() == (i.size * i.size * 4) as usize)
        );
    }

    #[test]
    fn missing_file_is_none() {
        assert_eq!(file_description(r"C:\nope\nope.exe"), None);
        assert_eq!(small_icon(r"C:\nope\nope.exe"), None);
    }
}
