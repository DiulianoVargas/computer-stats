//! CPU Temperature through PawnIO: the `AMDFamily17.bin` module on AMD (spike 001, verdict
//! Confirmed) or `IntelMSR.bin` on Intel (feature 004). Opened once for the app's lifetime.
//! `PawnIo` is shared with the motherboard Fan source (`superio.rs`, feature 006).

use std::ffi::{OsStr, c_char, c_void};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{CloseHandle, FreeLibrary, HANDLE, HMODULE};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{PCSTR, PCWSTR, s, w};

use crate::domain::cpu_temp::{
    self, CpuKind, MSR_PACKAGE_THERM_STATUS, MSR_TEMPERATURE_TARGET, MSR_THERM_STATUS,
    THM_TCON_CUR_TMP,
};
use crate::domain::metric::{Reading, Reason};

/// The CPU Temperature sensor: an open PawnIO handle with the module for this CPU loaded.
pub struct CpuTemperatureSource {
    pawnio: PawnIo,
    kind: CpuKind,
    /// Intel only: TjMax, read once at open.
    tjmax: f32,
}

impl CpuTemperatureSource {
    /// Startup checks in data-model order; a failure is permanent for the run.
    pub fn open() -> Result<CpuTemperatureSource, Reason> {
        if !is_elevated() {
            return Err(Reason::NotElevated);
        }
        let (vendor, family) = cpu_vendor_family();
        let (has_dts, has_ptm) = thermal_features();
        let kind =
            cpu_temp::cpu_kind(&vendor, family, has_dts, has_ptm).ok_or(Reason::UnsupportedCpu)?;
        let module_file = match kind {
            CpuKind::Amd => "AMDFamily17.bin",
            CpuKind::Intel { .. } => "IntelMSR.bin",
        };
        let module = std::env::current_exe()
            .map_err(|_| Reason::ModuleNotFound)?
            .with_file_name(module_file);
        let pawnio = PawnIo::open(&module)?;
        let tjmax = match kind {
            CpuKind::Amd => 0.0,
            CpuKind::Intel { .. } => {
                cpu_temp::intel_tjmax(pawnio.read_msr(MSR_TEMPERATURE_TARGET)?)
            }
        };
        Ok(CpuTemperatureSource {
            pawnio,
            kind,
            tjmax,
        })
    }

    pub fn read(&self) -> Reading {
        let reading = match self.kind {
            CpuKind::Amd => self
                .pawnio
                .execute(c"ioctl_read_smn", THM_TCON_CUR_TMP as u64)
                .map(|raw| cpu_temp::classify(raw as u32)),
            CpuKind::Intel { package: true } => self
                .pawnio
                .read_msr(MSR_PACKAGE_THERM_STATUS)
                .map(|status| cpu_temp::intel_package(status, self.tjmax)),
            CpuKind::Intel { package: false } => self
                .pawnio
                .read_msr(MSR_THERM_STATUS)
                .map(|status| cpu_temp::intel_core(status, self.tjmax)),
        };
        reading.unwrap_or_else(Reading::Unavailable)
    }
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
fn cpu_vendor_family() -> ([u8; 12], u32) {
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

/// CPUID leaf 6 EAX: digital thermal sensor (bit 0) and package thermal management (bit 6).
/// Both false when the CPU does not report leaf 6.
fn thermal_features() -> (bool, bool) {
    use core::arch::x86_64::__cpuid;
    // SAFETY: CPUID is available on every x86_64 CPU; leaf 6 is read only if reported.
    #[allow(unused_unsafe)]
    let max_leaf = unsafe { __cpuid(0) }.eax;
    if max_leaf < 6 {
        return (false, false);
    }
    #[allow(unused_unsafe)]
    let eax = unsafe { __cpuid(6) }.eax;
    (eax & 1 != 0, eax & (1 << 6) != 0)
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
pub(super) struct PawnIo {
    lib: HMODULE,
    handle: *mut c_void,
    execute: ExecuteFn,
    close: CloseFn,
}

impl PawnIo {
    /// Loads PawnIOLib.dll from the PawnIO install location and loads `module_path` into it.
    pub(super) fn open(module_path: &Path) -> Result<PawnIo, Reason> {
        // A missing module file is split from a missing driver (spike verdict).
        let blob = std::fs::read(module_path).map_err(|_| Reason::ModuleNotFound)?;
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

        // SAFETY: signatures match PawnIOLib's exports (spike research R1).
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

    /// Calls a module function with `input` cells, filling all of `output`; fails if the module
    /// returns an error or fewer cells than `output` holds.
    pub(super) fn call(
        &self,
        function: &std::ffi::CStr,
        input: &[u64],
        output: &mut [u64],
    ) -> Result<(), Reason> {
        let mut returned = 0usize;
        // SAFETY: buffers are valid for the lengths passed; the function name is NUL-terminated.
        let hr = unsafe {
            (self.execute)(
                self.handle,
                function.as_ptr(),
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
                output.len(),
                &mut returned,
            )
        };
        if hr < 0 || returned != output.len() {
            return Err(Reason::ReadFailed);
        }
        Ok(())
    }

    /// Calls a module function that takes one input cell and returns one output cell (the AMD
    /// module's `ioctl_read_smn`, the Intel module's `ioctl_read_msr`).
    fn execute(&self, function: &std::ffi::CStr, input: u64) -> Result<u64, Reason> {
        let mut output = [0u64; 1];
        self.call(function, &[input], &mut output)?;
        Ok(output[0])
    }

    /// Reads one MSR through the Intel module (only its whitelisted thermal MSRs succeed).
    fn read_msr(&self, msr: u32) -> Result<u64, Reason> {
        self.execute(c"ioctl_read_msr", msr as u64)
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
