//! GPU Usage, GPU Temperature and VRAM through NVML, loaded at runtime from System32 so that
//! "no GPU" is a normal state (research R9). Device index 0.

use std::ffi::c_void;

use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use windows::core::{PCSTR, s, w};

use crate::domain::metric::{MetricKind, Reading, Reason};

type NvmlReturn = i32;
type Device = *mut c_void;

const NVML_SUCCESS: NvmlReturn = 0;
const NVML_ERROR_DRIVER_NOT_LOADED: NvmlReturn = 9;
const NVML_TEMPERATURE_GPU: u32 = 0;

#[repr(C)]
#[derive(Default)]
struct Utilization {
    gpu: u32,
    memory: u32,
}

#[repr(C)]
#[derive(Default)]
struct Memory {
    total: u64,
    free: u64,
    used: u64,
}

type InitFn = unsafe extern "C" fn() -> NvmlReturn;
type ShutdownFn = unsafe extern "C" fn() -> NvmlReturn;
type GetCountFn = unsafe extern "C" fn(*mut u32) -> NvmlReturn;
type GetHandleFn = unsafe extern "C" fn(u32, *mut Device) -> NvmlReturn;
type GetUtilizationFn = unsafe extern "C" fn(Device, *mut Utilization) -> NvmlReturn;
type GetMemoryFn = unsafe extern "C" fn(Device, *mut Memory) -> NvmlReturn;
type GetTemperatureFn = unsafe extern "C" fn(Device, u32, *mut u32) -> NvmlReturn;

/// The three GPU Metrics of one tick.
pub struct GpuReadings {
    pub usage: Reading,
    pub temperature: Reading,
    pub vram: Reading,
}

impl GpuReadings {
    pub fn unavailable(reason: Reason) -> GpuReadings {
        GpuReadings {
            usage: Reading::Unavailable(reason),
            temperature: Reading::Unavailable(reason),
            vram: Reading::Unavailable(reason),
        }
    }
}

/// An initialized NVML with the first GPU. Shut down on drop.
pub struct GpuSource {
    lib: HMODULE,
    device: Device,
    shutdown: ShutdownFn,
    utilization: GetUtilizationFn,
    memory: GetMemoryFn,
    temperature: GetTemperatureFn,
}

impl GpuSource {
    pub fn open() -> Result<GpuSource, Reason> {
        // SAFETY: literal, NUL-terminated name; System32-only search (no DLL planting).
        let lib = unsafe { LoadLibraryExW(w!("nvml.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }
            .map_err(|_| Reason::NoGpu)?;
        let fail = |reason| {
            // SAFETY: `lib` was loaded above and not yet freed.
            let _ = unsafe { FreeLibrary(lib) };
            Err(reason)
        };

        // SAFETY: signatures match NVML's documented exports.
        let fns = unsafe {
            (
                proc::<InitFn>(lib, s!("nvmlInit_v2")),
                proc::<ShutdownFn>(lib, s!("nvmlShutdown")),
                proc::<GetCountFn>(lib, s!("nvmlDeviceGetCount_v2")),
                proc::<GetHandleFn>(lib, s!("nvmlDeviceGetHandleByIndex_v2")),
                proc::<GetUtilizationFn>(lib, s!("nvmlDeviceGetUtilizationRates")),
                proc::<GetMemoryFn>(lib, s!("nvmlDeviceGetMemoryInfo")),
                proc::<GetTemperatureFn>(lib, s!("nvmlDeviceGetTemperature")),
            )
        };
        let (
            Some(init),
            Some(shutdown),
            Some(count),
            Some(handle),
            Some(utilization),
            Some(memory),
            Some(temperature),
        ) = fns
        else {
            return fail(Reason::DriverNotFound);
        };

        // SAFETY: NVML calls with valid out-pointers; shutdown pairs with a successful init.
        unsafe {
            match init() {
                NVML_SUCCESS => {}
                NVML_ERROR_DRIVER_NOT_LOADED => return fail(Reason::DriverNotFound),
                _ => return fail(Reason::NoGpu),
            }
            let mut n = 0u32;
            let mut device: Device = std::ptr::null_mut();
            if count(&mut n) != NVML_SUCCESS || n == 0 || handle(0, &mut device) != NVML_SUCCESS {
                shutdown();
                return fail(Reason::NoGpu);
            }
            Ok(GpuSource {
                lib,
                device,
                shutdown,
                utilization,
                memory,
                temperature,
            })
        }
    }

    /// Each Metric fails independently with `ReadFailed`.
    pub fn read(&self) -> GpuReadings {
        let mut util = Utilization::default();
        let mut mem = Memory::default();
        let mut temp = 0u32;
        // SAFETY: `device` is valid while NVML is initialized; out-pointers are valid.
        let (u, m, t) = unsafe {
            (
                (self.utilization)(self.device, &mut util),
                (self.memory)(self.device, &mut mem),
                (self.temperature)(self.device, NVML_TEMPERATURE_GPU, &mut temp),
            )
        };
        let failed = Reading::Unavailable(Reason::ReadFailed);
        GpuReadings {
            usage: if u == NVML_SUCCESS {
                Reading::of(MetricKind::GpuUsage, util.gpu as f32)
            } else {
                failed
            },
            temperature: if t == NVML_SUCCESS {
                Reading::of(MetricKind::GpuTemperature, temp as f32)
            } else {
                failed
            },
            vram: if m == NVML_SUCCESS && mem.total > 0 {
                let pct = (mem.used as f64 / mem.total as f64 * 100.0) as f32;
                Reading::validated(MetricKind::Vram, pct, Some(mem.used), Some(mem.total))
            } else {
                failed
            },
        }
    }
}

impl Drop for GpuSource {
    fn drop(&mut self) {
        // SAFETY: NVML was initialized in `open`; the library is freed exactly once.
        unsafe {
            (self.shutdown)();
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
