//! Memory Usage from `GlobalMemoryStatusEx` (research R8). System RAM only, never VRAM.

use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

use crate::domain::metric::{MetricKind, Reading, Reason};

pub fn memory_usage() -> Reading {
    let mut status = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `status` is a valid MEMORYSTATUSEX with `dwLength` set.
    if unsafe { GlobalMemoryStatusEx(&mut status) }.is_err() || status.ullTotalPhys == 0 {
        return Reading::Unavailable(Reason::ReadFailed);
    }
    let total = status.ullTotalPhys;
    let used = total.saturating_sub(status.ullAvailPhys);
    let pct = (used as f64 / total as f64 * 100.0) as f32;
    Reading::validated(MetricKind::MemoryUsage, pct, Some(used), Some(total))
}
