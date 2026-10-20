//! CPU Usage from `GetSystemTimes` deltas (research R8).

use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::GetSystemTimes;

use crate::domain::metric::{MetricKind, Reading, Reason};

#[derive(Clone, Copy)]
struct Times {
    idle: u64,
    kernel: u64,
    user: u64,
}

/// Remembers the previous system times; usage is the share of non-idle time between calls.
pub struct CpuTimes {
    previous: Option<Times>,
}

impl CpuTimes {
    pub fn new() -> CpuTimes {
        CpuTimes { previous: read() }
    }

    /// CPU Usage since the previous call. The first call after a failed read is unavailable.
    pub fn sample(&mut self) -> Reading {
        let current = read();
        let reading = match (self.previous, current) {
            (Some(a), Some(b)) => {
                let idle = b.idle.wrapping_sub(a.idle);
                // Kernel time includes idle time.
                let total = b.kernel.wrapping_sub(a.kernel) + b.user.wrapping_sub(a.user);
                if total == 0 {
                    Reading::Unavailable(Reason::ReadFailed)
                } else {
                    let busy = 1.0 - idle as f64 / total as f64;
                    Reading::of(MetricKind::CpuUsage, (busy * 100.0) as f32)
                }
            }
            _ => Reading::Unavailable(Reason::ReadFailed),
        };
        self.previous = current;
        reading
    }
}

fn read() -> Option<Times> {
    let (mut idle, mut kernel, mut user) = Default::default();
    // SAFETY: all three out-pointers are valid FILETIMEs.
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
    Some(Times {
        idle: ticks(idle),
        kernel: ticks(kernel),
        user: ticks(user),
    })
}

fn ticks(t: FILETIME) -> u64 {
    ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64
}
