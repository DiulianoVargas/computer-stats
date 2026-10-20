//! Drives: fixed, non-USB volumes with a drive letter, and their Disk Space (research R8; FR-022).

use std::ffi::c_void;

use windows::Win32::Foundation::{CloseHandle, GENERIC_ACCESS_RIGHTS};
use windows::Win32::Storage::FileSystem::{
    BusTypeUsb, CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
    GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::{
    IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery, STORAGE_DEVICE_DESCRIPTOR,
    STORAGE_PROPERTY_QUERY, StorageDeviceProperty,
};
use windows::core::PCWSTR;

use crate::domain::drive::Drive;
use crate::domain::metric::Reason;

/// `DRIVE_FIXED` from `GetDriveTypeW`.
const DRIVE_FIXED: u32 = 3;

/// Letters of fixed, non-USB volumes, sorted.
pub fn enumerate() -> Vec<char> {
    // SAFETY: no preconditions.
    let mask = unsafe { GetLogicalDrives() };
    (0..26u8)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| (b'A' + i) as char)
        .filter(|&letter| is_fixed(letter) && !is_usb(letter))
        .collect()
}

/// Reads a Drive's label and Disk Space.
pub fn read(letter: char) -> Result<Drive, Reason> {
    let root = wide(&format!("{letter}:\\"));
    let (mut free, mut total) = (0u64, 0u64);
    // SAFETY: `root` is NUL-terminated; out-pointers are valid u64s.
    unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(root.as_ptr()),
            None,
            Some(&mut total),
            Some(&mut free),
        )
    }
    .map_err(|_| Reason::ReadFailed)?;

    let mut label = [0u16; 261];
    // SAFETY: `root` is NUL-terminated; `label` is a valid buffer of its length.
    let ok = unsafe {
        GetVolumeInformationW(
            PCWSTR(root.as_ptr()),
            Some(&mut label),
            None,
            None,
            None,
            None,
        )
    }
    .is_ok();
    let label = if ok {
        let len = label.iter().position(|&c| c == 0).unwrap_or(label.len());
        String::from_utf16_lossy(&label[..len])
    } else {
        String::new()
    };

    Ok(Drive {
        letter,
        label,
        total_bytes: total,
        free_bytes: free,
    })
}

fn is_fixed(letter: char) -> bool {
    let root = wide(&format!("{letter}:\\"));
    // SAFETY: `root` is NUL-terminated.
    unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) == DRIVE_FIXED }
}

/// Whether the volume sits on a USB bus (external disks report as fixed). Unknown → not USB.
fn is_usb(letter: char) -> bool {
    let path = wide(&format!(r"\\.\{letter}:"));
    // SAFETY: `path` is NUL-terminated; zero access rights only query device properties.
    let Ok(device) = (unsafe {
        CreateFileW(
            PCWSTR(path.as_ptr()),
            GENERIC_ACCESS_RIGHTS(0).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }) else {
        return false;
    };

    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        ..Default::default()
    };
    let mut descriptor = STORAGE_DEVICE_DESCRIPTOR::default();
    let mut returned = 0u32;
    // SAFETY: input/output buffers are valid for the sizes passed; `device` is open.
    let ok = unsafe {
        DeviceIoControl(
            device,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&query as *const _ as *const c_void),
            size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            Some(&mut descriptor as *mut _ as *mut c_void),
            size_of::<STORAGE_DEVICE_DESCRIPTOR>() as u32,
            Some(&mut returned),
            None,
        )
    }
    .is_ok();
    // SAFETY: `device` is a handle we own.
    let _ = unsafe { CloseHandle(device) };
    ok && descriptor.BusType == BusTypeUsb
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
