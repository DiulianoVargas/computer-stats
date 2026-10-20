//! The only module allowed to use `unsafe` (Constitution III). Every function here exposes a
//! safe API and maps every failure to a `Reason` — nothing panics.

use std::ffi::{OsStr, c_char, c_void};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{CloseHandle, FreeLibrary, HANDLE, HMODULE};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Console::{
    CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{BOOL, PCSTR, PCWSTR, s, w};

use crate::reading::{Clock, Reason};

// --- Clock & Ctrl+C ----------------------------------------------------------

pub fn local_clock() -> Clock {
    // SAFETY: GetLocalTime has no preconditions.
    let t = unsafe { GetLocalTime() };
    Clock {
        h: t.wHour as u8,
        m: t.wMinute as u8,
        s: t.wSecond as u8,
    }
}

static STOP: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn on_ctrl(event: u32) -> BOOL {
    if matches!(event, CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT) {
        STOP.store(true, Ordering::SeqCst);
        return BOOL(1);
    }
    BOOL(0)
}

/// Routes Ctrl+C / Ctrl+Break / console close to `stop_requested()` instead of killing the process.
pub fn install_ctrl_handler() {
    // SAFETY: `on_ctrl` is a valid handler for the life of the process. Failure only means
    // Ctrl+C falls back to the default (non-zero exit), which is acceptable.
    let _ = unsafe { SetConsoleCtrlHandler(Some(on_ctrl), true) };
}

pub fn stop_requested() -> bool {
    STOP.load(Ordering::SeqCst)
}

// --- Elevation & CPUID -------------------------------------------------------

pub fn is_elevated() -> bool {
    let mut token = HANDLE::default();
    // SAFETY: pseudo-handle from GetCurrentProcess needs no closing; `token` is closed below.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.is_err() {
        return false;
    }
    let mut elevation = TOKEN_ELEVATION::default();
    let mut len = 0u32;
    // SAFETY: buffer is a TOKEN_ELEVATION of exactly the size passed.
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut c_void),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
    }
    .is_ok();
    // SAFETY: `token` is a valid handle we own.
    let _ = unsafe { CloseHandle(token) };
    ok && elevation.TokenIsElevated != 0
}

/// Returns the CPUID vendor string and the effective family (base + extended).
pub fn cpu_vendor_family() -> ([u8; 12], u32) {
    use core::arch::x86_64::__cpuid;
    // SAFETY: CPUID is available on every x86_64 CPU.
    #[allow(unused_unsafe)]
    let (leaf0, leaf1) = unsafe { (__cpuid(0), __cpuid(1)) };

    let mut vendor = [0u8; 12];
    vendor[0..4].copy_from_slice(&leaf0.ebx.to_le_bytes());
    vendor[4..8].copy_from_slice(&leaf0.edx.to_le_bytes());
    vendor[8..12].copy_from_slice(&leaf0.ecx.to_le_bytes());

    let base = (leaf1.eax >> 8) & 0xF;
    let family = if base == 0xF {
        base + ((leaf1.eax >> 20) & 0xFF)
    } else {
        base
    };
    (vendor, family)
}

// --- PawnIO ------------------------------------------------------------------

type Hresult = i32;
const E_ACCESSDENIED: Hresult = 0x8007_0005_u32 as i32;

type OpenFn = unsafe extern "system" fn(*mut *mut c_void) -> Hresult;
type LoadFn = unsafe extern "system" fn(*mut c_void, *const u8, usize) -> Hresult;
type ExecuteFn = unsafe extern "system" fn(
    *mut c_void,
    *const c_char,
    *const u64,
    usize,
    *mut u64,
    usize,
    *mut usize,
) -> Hresult;
type CloseFn = unsafe extern "system" fn(*mut c_void) -> Hresult;

/// An open PawnIO handle with a loaded module. Closed on drop.
pub struct PawnIo {
    lib: HMODULE,
    handle: *mut c_void,
    execute: ExecuteFn,
    close: CloseFn,
}

impl PawnIo {
    /// Loads PawnIOLib.dll from the PawnIO install location and loads `module_path` into it.
    pub fn open(module_path: &Path) -> Result<PawnIo, Reason> {
        let blob = std::fs::read(module_path).map_err(|_| Reason::DriverNotFound)?;
        let dll = to_wide(install_dir().join("PawnIOLib.dll").as_os_str());

        // SAFETY: `dll` is a NUL-terminated UTF-16 path.
        let lib =
            unsafe { LoadLibraryW(PCWSTR(dll.as_ptr())) }.map_err(|_| Reason::DriverNotFound)?;

        // From here on, any early return must free `lib`.
        let fail = |reason| {
            // SAFETY: `lib` was returned by LoadLibraryW and not yet freed.
            let _ = unsafe { FreeLibrary(lib) };
            Err(reason)
        };

        // SAFETY: signatures match PawnIOLib's exports (research R1).
        let fns = unsafe {
            (
                proc::<OpenFn>(lib, s!("pawnio_open")),
                proc::<LoadFn>(lib, s!("pawnio_load")),
                proc::<ExecuteFn>(lib, s!("pawnio_execute")),
                proc::<CloseFn>(lib, s!("pawnio_close")),
            )
        };
        let (Some(open), Some(load), Some(execute), Some(close)) = fns else {
            return fail(Reason::DriverNotFound);
        };

        let mut handle: *mut c_void = std::ptr::null_mut();
        // SAFETY: `handle` is a valid out-pointer.
        let hr = unsafe { open(&mut handle) };
        if hr < 0 {
            return fail(if hr == E_ACCESSDENIED {
                Reason::NotElevated
            } else {
                Reason::DriverNotFound
            });
        }

        // SAFETY: `handle` is open; `blob` outlives the call.
        let hr = unsafe { load(handle, blob.as_ptr(), blob.len()) };
        if hr < 0 {
            // SAFETY: `handle` is open and closed exactly once here.
            unsafe { close(handle) };
            return fail(Reason::DriverNotFound);
        }

        Ok(PawnIo {
            lib,
            handle,
            execute,
            close,
        })
    }

    /// Reads one 32-bit SMN register through the module's `ioctl_read_smn`.
    pub fn read_smn(&self, offset: u32) -> Result<u32, Reason> {
        let input = [offset as u64];
        let mut output = [0u64; 1];
        let mut returned = 0usize;
        // SAFETY: buffers are valid for the lengths passed; the function name is NUL-terminated.
        let hr = unsafe {
            (self.execute)(
                self.handle,
                c"ioctl_read_smn".as_ptr(),
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
                output.len(),
                &mut returned,
            )
        };
        if hr < 0 || returned != 1 {
            return Err(Reason::ReadFailed);
        }
        Ok(output[0] as u32)
    }
}

impl Drop for PawnIo {
    fn drop(&mut self) {
        // SAFETY: `handle` and `lib` are valid and released exactly once.
        unsafe {
            (self.close)(self.handle);
            let _ = FreeLibrary(self.lib);
        }
    }
}

/// # Safety
/// `F` must be the exact function-pointer type of the export.
unsafe fn proc<F: Copy>(lib: HMODULE, name: PCSTR) -> Option<F> {
    // SAFETY: caller guarantees `F` matches the export; FARPROC and `F` are both fn pointers.
    unsafe { GetProcAddress(lib, name).map(|f| std::mem::transmute_copy(&f)) }
}

/// PawnIO's install directory from its uninstall registry key, or the default location.
fn install_dir() -> PathBuf {
    let mut buf = [0u16; 260];
    let mut size = size_of_val(&buf) as u32;
    // SAFETY: `buf` and `size` describe a valid writable buffer.
    let err = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO"),
            w!("InstallLocation"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut c_void),
            Some(&mut size),
        )
    };
    if err.is_ok() {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        if len > 0 {
            return PathBuf::from(String::from_utf16_lossy(&buf[..len]));
        }
    }
    PathBuf::from(r"C:\Program Files\PawnIO")
}

fn to_wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}
