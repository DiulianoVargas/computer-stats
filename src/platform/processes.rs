//! Processes for the Processes View: one `NtQuerySystemInformation` call per tick (003 research
//! R1), program paths for new Processes (R4), and End App with a creation-time check (R7).

use std::ffi::c_void;

use windows::Wdk::System::SystemInformation::{NtQuerySystemInformation, SystemProcessInformation};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, FILETIME, GetLastError, HANDLE, NTSTATUS,
};
use windows::Win32::System::Threading::{
    GetActiveProcessorCount, GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, QueryFullProcessImageNameW,
    TerminateProcess,
};
use windows::core::PWSTR;

use crate::domain::apps::{ProcessId, ProcessSample};

const STATUS_INFO_LENGTH_MISMATCH: NTSTATUS = NTSTATUS(0xC000_0004_u32 as i32);
const ALL_PROCESSOR_GROUPS: u16 = 0xFFFF;

/// The leading fields of `SYSTEM_PROCESS_INFORMATION` (x64 layout), which the `windows` crate
/// only exposes with most of them reserved.
#[repr(C)]
#[derive(Clone, Copy)]
struct Entry {
    next_entry_offset: u32,
    number_of_threads: u32,
    working_set_private_size: i64,
    hard_fault_count: u32,
    number_of_threads_high_watermark: u32,
    cycle_time: u64,
    create_time: i64,
    user_time: i64,
    kernel_time: i64,
    image_name_length: u16,
    image_name_maximum_length: u16,
    image_name_buffer: *const u16,
    base_priority: i32,
    unique_process_id: usize,
}

/// Owns the buffer reused between ticks; dropped (freeing it) while the View is hidden.
#[derive(Default)]
pub struct Enumerator {
    /// `u64` for 8-byte alignment of the entries.
    buffer: Vec<u64>,
}

impl Enumerator {
    /// Every Process (including PID 0, which the caller skips), without paths.
    pub fn sample(&mut self) -> Option<Vec<ProcessSample>> {
        let len = self.query()?;
        let bytes = len.min(self.buffer.len() * 8);
        let base = self.buffer.as_ptr() as *const u8;
        let mut out = Vec::with_capacity(400);
        let mut offset = 0usize;
        loop {
            if offset + size_of::<Entry>() > bytes {
                break;
            }
            // SAFETY: `offset + size_of::<Entry>()` is within the bytes the call filled.
            let entry = unsafe { (base.add(offset) as *const Entry).read_unaligned() };
            out.push(ProcessSample {
                id: ProcessId {
                    pid: entry.unique_process_id as u32,
                    create_time: entry.create_time as u64,
                },
                image_name: image_name(&entry, base, bytes),
                path: None,
                cpu_time: (entry.user_time as u64).saturating_add(entry.kernel_time as u64),
                private_bytes: entry.working_set_private_size.max(0) as u64,
            });
            if entry.next_entry_offset == 0 {
                break;
            }
            offset += entry.next_entry_offset as usize;
        }
        Some(out)
    }

    /// Fills the buffer, growing it as Windows asks. Returns the filled length in bytes.
    fn query(&mut self) -> Option<usize> {
        if self.buffer.is_empty() {
            self.buffer = vec![0; 512 * 1024 / 8];
        }
        for _ in 0..5 {
            let mut needed = 0u32;
            // SAFETY: the buffer is valid for the length passed; `needed` is a valid out-pointer.
            let status = unsafe {
                NtQuerySystemInformation(
                    SystemProcessInformation,
                    self.buffer.as_mut_ptr() as *mut c_void,
                    (self.buffer.len() * 8) as u32,
                    &mut needed,
                )
            };
            if status == STATUS_INFO_LENGTH_MISMATCH {
                // Processes may start between calls: leave some room.
                let words = (needed as usize + 64 * 1024).div_ceil(8);
                self.buffer = vec![0; words];
                continue;
            }
            return status.is_ok().then_some(needed as usize);
        }
        None
    }
}

/// The image name, if its buffer lies inside the filled data (it always does in practice).
fn image_name(entry: &Entry, base: *const u8, bytes: usize) -> String {
    let start = entry.image_name_buffer as usize;
    let len = entry.image_name_length as usize;
    let lo = base as usize;
    if start == 0 || len == 0 || start < lo || start + len > lo + bytes || !start.is_multiple_of(2)
    {
        return if entry.unique_process_id == 0 {
            "System Idle Process".into()
        } else {
            String::new()
        };
    }
    // SAFETY: the range was checked to lie inside the buffer and is u16-aligned.
    let units = unsafe { std::slice::from_raw_parts(entry.image_name_buffer, len / 2) };
    String::from_utf16_lossy(units)
}

/// Logical processors across all groups (CPU % capacity).
pub fn logical_cpus() -> u32 {
    // SAFETY: no preconditions.
    unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) }.max(1)
}

/// The Process's program file path, or `None` (protected/pseudo Processes, or it exited).
pub fn image_path(pid: u32) -> Option<String> {
    let handle = open(pid, PROCESS_QUERY_LIMITED_INFORMATION.0).ok()?;
    let mut buf = [0u16; 1024];
    let mut size = buf.len() as u32;
    // SAFETY: `buf` and `size` describe a valid writable buffer; `handle` is open.
    let ok = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
    }
    .is_ok();
    close(handle);
    ok.then(|| String::from_utf16_lossy(&buf[..size as usize]))
}

/// What happened when Ending an App.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EndOutcome {
    pub ended: usize,
    pub denied: usize,
    /// Already exited, or the PID now belongs to another Process (never ended).
    pub gone: usize,
}

/// Forcibly ends each Process, but only if it is still the same Process that was sampled (same
/// creation time), so a reused PID is never ended.
pub fn end(processes: &[ProcessId]) -> EndOutcome {
    let mut outcome = EndOutcome::default();
    for id in processes {
        let handle = match open(
            id.pid,
            PROCESS_TERMINATE.0 | PROCESS_QUERY_LIMITED_INFORMATION.0,
        ) {
            Ok(handle) => handle,
            Err(denied) => {
                if denied {
                    outcome.denied += 1;
                } else {
                    outcome.gone += 1;
                }
                continue;
            }
        };
        if creation_time(handle) != Some(id.create_time) {
            outcome.gone += 1;
        } else if unsafe_terminate(handle) {
            outcome.ended += 1;
        } else {
            outcome.denied += 1;
        }
        close(handle);
    }
    outcome
}

/// Opens a Process; the error is `true` for "access denied", `false` for "no such Process".
fn open(pid: u32, access: u32) -> Result<HANDLE, bool> {
    // SAFETY: plain call; the handle is closed by the caller.
    unsafe {
        OpenProcess(
            windows::Win32::System::Threading::PROCESS_ACCESS_RIGHTS(access),
            false,
            pid,
        )
    }
    .map_err(|_| {
        // SAFETY: read right after the failing call.
        let error = unsafe { GetLastError() };
        error == ERROR_ACCESS_DENIED
    })
}

fn creation_time(handle: HANDLE) -> Option<u64> {
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: valid out-pointers; `handle` has query rights.
    unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) }.ok()?;
    Some(((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64)
}

fn unsafe_terminate(handle: HANDLE) -> bool {
    // SAFETY: `handle` has PROCESS_TERMINATE and was verified to be the sampled Process.
    unsafe { TerminateProcess(handle, 1) }.is_ok()
}

fn close(handle: HANDLE) {
    // SAFETY: `handle` is open and owned by the caller.
    let _ = unsafe { CloseHandle(handle) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_layout_matches_windows() {
        // Offsets of SYSTEM_PROCESS_INFORMATION on x64.
        assert_eq!(std::mem::offset_of!(Entry, working_set_private_size), 8);
        assert_eq!(std::mem::offset_of!(Entry, create_time), 32);
        assert_eq!(std::mem::offset_of!(Entry, kernel_time), 48);
        assert_eq!(std::mem::offset_of!(Entry, image_name_buffer), 64);
        assert_eq!(std::mem::offset_of!(Entry, unique_process_id), 80);
    }

    #[test]
    fn sees_this_process() {
        let mut e = Enumerator::default();
        let all = e.sample().unwrap_or_default();
        let me = std::process::id();
        let mine = all.iter().find(|p| p.id.pid == me);
        assert!(mine.is_some_and(|p| p.private_bytes > 0 && !p.image_name.is_empty()));
        assert!(image_path(me).is_some_and(|p| p.to_lowercase().ends_with(".exe")));
        assert!(logical_cpus() >= 1);
    }

    #[test]
    fn end_skips_a_process_whose_creation_time_differs() {
        let me = ProcessId {
            pid: std::process::id(),
            create_time: 1,
        };
        // Same PID, wrong creation time: treated as "gone", never terminated.
        assert_eq!(
            end(&[me]),
            EndOutcome {
                ended: 0,
                denied: 0,
                gone: 1
            }
        );
    }

    #[test]
    fn sampled_create_time_matches_get_process_times() {
        let me = std::process::id();
        let sampled = Enumerator::default()
            .sample()
            .and_then(|all| all.into_iter().find(|p| p.id.pid == me))
            .map(|p| p.id.create_time);
        let actual = open(me, PROCESS_QUERY_LIMITED_INFORMATION.0)
            .ok()
            .and_then(|h| {
                let t = creation_time(h);
                close(h);
                t
            });
        assert!(sampled.is_some());
        assert_eq!(sampled, actual);
    }

    #[test]
    fn ends_a_child_process_found_by_enumeration() {
        let Ok(mut child) = std::process::Command::new("cmd.exe")
            .args(["/c", "ping -n 30 127.0.0.1 > nul"])
            .spawn()
        else {
            return;
        };
        let pid = child.id();
        let id = Enumerator::default()
            .sample()
            .and_then(|all| all.into_iter().find(|p| p.id.pid == pid))
            .map(|p| p.id);
        assert!(id.is_some(), "child not enumerated");
        let outcome = end(&id.into_iter().collect::<Vec<_>>());
        assert_eq!(outcome.ended, 1, "{outcome:?}");
        let status = child.wait().map(|s| s.code());
        assert_eq!(
            status.ok().flatten(),
            Some(1),
            "exit code is TerminateProcess's"
        );
    }
}
