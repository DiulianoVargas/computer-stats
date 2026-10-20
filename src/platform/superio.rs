//! Motherboard Fan Speeds from the Nuvoton Super I/O chip through PawnIO's `LpcIO.bin` module
//! (006 research R2–R5). Read-only: the only writes are the chip's access protocol (configuration
//! mode, device / bank / register select) and clearing the firmware's I/O space read lock, as
//! HWiNFO and iCUE do (FR-006, revised). No fan, curve or voltage register is ever written.

use std::ffi::c_void;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};
use windows::core::w;

use super::pawnio::{PawnIo, is_elevated};
use crate::domain::fan::{self, FanChip, FanSample};
use crate::domain::metric::Reason;

/// How long a tick may wait for another monitoring tool to release the chip (research R5).
const MUTEX_WAIT_MS: u32 = 10;
/// How often a chip that stays locked (or unsupported) is checked again (research R4).
const LOCK_RECHECK: Duration = Duration::from_secs(60);

// Super I/O configuration registers (Nuvoton).
const CONFIG_PORTS: [u16; 2] = [0x2E, 0x4E];
const ENTER_KEY: u8 = 0x87;
const EXIT_KEY: u8 = 0xAA;
const DEVICE_SELECT: u8 = 0x07;
const CHIP_ID: u8 = 0x20;
const CHIP_REVISION: u8 = 0x21;
const BASE_ADDRESS: u8 = 0x60;
const IO_SPACE_LOCK: u8 = 0x28;
const IO_SPACE_LOCKED: u8 = 0x10;
const HARDWARE_MONITOR_DEVICE: u8 = 0x0B;

// Hardware monitor I/O space.
const INDEX_OFFSET: u16 = 0x05;
const DATA_OFFSET: u16 = 0x06;
const BANK_SELECT: u8 = 0x4E;

/// The motherboard Fan source, opened once on the sampler thread.
pub struct FanSource {
    pawnio: PawnIo,
    mutex: IsaBusMutex,
    state: State,
}

enum State {
    /// `port`: the configuration port the chip answered on, still selected in the module.
    Ready { port: u16, base: u16, chip: FanChip },
    /// Detection is due again at `at`; until then every read gives `reason`.
    Retry { at: Instant, reason: Reason },
}

enum Detection {
    Ready { port: u16, base: u16, chip: FanChip },
    Locked,
    Unsupported,
}

impl FanSource {
    /// Startup checks; `Err` is permanent for the run, like CPU Temperature.
    pub fn open() -> Result<FanSource, Reason> {
        if !is_elevated() {
            return Err(Reason::NotElevated);
        }
        let module = std::env::current_exe()
            .map_err(|_| Reason::ModuleNotFound)?
            .with_file_name("LpcIO.bin");
        let pawnio = PawnIo::open(&module)?;
        let mutex = IsaBusMutex::open().ok_or(Reason::ReadFailed)?;
        let mut source = FanSource {
            pawnio,
            mutex,
            state: State::Retry {
                at: Instant::now(),
                reason: Reason::ReadFailed,
            },
        };
        source.detect_now()?;
        Ok(source)
    }

    /// One sample per channel, or why the Fans cannot be read this tick.
    pub fn read(&mut self) -> Result<Vec<FanSample>, Reason> {
        let (port, base, chip) = match self.state {
            State::Ready { port, base, chip } => (port, base, chip),
            State::Retry { at, reason } => {
                if Instant::now() < at {
                    return Err(reason);
                }
                self.detect_now()?;
                match self.state {
                    State::Ready { port, base, chip } => (port, base, chip),
                    State::Retry { reason, .. } => return Err(reason),
                }
            }
        };

        let Some(_guard) = self.mutex.lock() else {
            // Another tool holds the chip: a gap for every Fan this second.
            return Ok(vec![FanSample::Invalid; chip.channels.len()]);
        };
        let raw = self.read_counts(base, chip)?;
        // Every byte 0xFF: the firmware locked the chip again (e.g. after sleep). This second is a
        // gap; the next tick re-runs detection, which clears the lock again.
        if raw.iter().all(|&(high, low)| high == 0xFF && low == 0xFF) && self.is_locked(port)? {
            self.state = State::Retry {
                at: Instant::now(),
                reason: Reason::LockedByFirmware,
            };
            return Ok(vec![FanSample::Invalid; chip.channels.len()]);
        }
        Ok(raw
            .into_iter()
            .map(|(high, low)| fan::fan_speed(high, low))
            .collect())
    }

    /// Runs detection and updates `state`. `Err` only for an unsupported chip (`open` turns it
    /// into a permanent failure).
    fn detect_now(&mut self) -> Result<(), Reason> {
        let detection = match self.mutex.lock() {
            Some(_guard) => self.detect(),
            // Busy: try again next second.
            None => {
                self.state = State::Retry {
                    at: Instant::now() + Duration::from_secs(1),
                    reason: Reason::ReadFailed,
                };
                return Ok(());
            }
        };
        self.state = match detection {
            Ok(Detection::Ready { port, base, chip }) => State::Ready { port, base, chip },
            Ok(Detection::Locked) => State::Retry {
                at: Instant::now() + LOCK_RECHECK,
                reason: Reason::LockedByFirmware,
            },
            Ok(Detection::Unsupported) => {
                // Checked again only as rarely as a locked chip.
                self.state = State::Retry {
                    at: Instant::now() + LOCK_RECHECK,
                    reason: Reason::UnsupportedMotherboard,
                };
                return Err(Reason::UnsupportedMotherboard);
            }
            Err(reason) => State::Retry {
                at: Instant::now() + LOCK_RECHECK,
                reason,
            },
        };
        Ok(())
    }

    /// Finds a supported chip on either configuration port. The caller holds the ISA bus mutex.
    fn detect(&self) -> Result<Detection, Reason> {
        for (slot, &port) in CONFIG_PORTS.iter().enumerate() {
            self.pawnio
                .call(c"ioctl_select_slot", &[slot as u64], &mut [])?;
            self.port_out(port, ENTER_KEY)?;
            self.port_out(port, ENTER_KEY)?;
            let found = self.identify(port);
            // Always leave configuration mode, whatever was found.
            let exit = self.port_out(port, EXIT_KEY);
            match found? {
                Some(detection) => {
                    exit?;
                    return Ok(detection);
                }
                None => continue,
            }
        }
        Ok(Detection::Unsupported)
    }

    /// Inside configuration mode: the chip and its hardware monitor base, clearing the read lock if
    /// the firmware set it; `None` when this port has no supported chip.
    fn identify(&self, port: u16) -> Result<Option<Detection>, Reason> {
        let id = self.config_in(CHIP_ID)?;
        let revision = self.config_in(CHIP_REVISION)?;
        let Some(chip) = fan::fan_chip(id, revision) else {
            return Ok(None);
        };
        // Whitelists the devices' I/O ranges for `ioctl_pio_*`.
        self.pawnio.call(c"ioctl_find_bars", &[], &mut [])?;
        self.config_out(DEVICE_SELECT, HARDWARE_MONITOR_DEVICE)?;
        let base = self.config_in_word(BASE_ADDRESS)?;
        std::thread::sleep(Duration::from_millis(1));
        let verify = self.config_in_word(BASE_ADDRESS)?;
        if base != verify || !fan::is_valid_base(base) {
            // Disabled or relocated by the firmware; enabling it would be a setting change.
            return Ok(Some(Detection::Unsupported));
        }
        let lock = self.config_in(IO_SPACE_LOCK)?;
        if lock & IO_SPACE_LOCKED != 0 {
            // The one setting the app changes (FR-006, revised; the user's decision): only this
            // bit, read-modify-write. The firmware sets it again on the next boot.
            self.config_out(IO_SPACE_LOCK, lock & !IO_SPACE_LOCKED)?;
            if self.config_in(IO_SPACE_LOCK)? & IO_SPACE_LOCKED != 0 {
                return Ok(Some(Detection::Locked));
            }
        }
        Ok(Some(Detection::Ready { port, base, chip }))
    }

    /// Re-reads only the lock bit (the slot and its I/O ranges are still selected). The caller
    /// holds the mutex.
    fn is_locked(&self, port: u16) -> Result<bool, Reason> {
        self.port_out(port, ENTER_KEY)?;
        self.port_out(port, ENTER_KEY)?;
        let lock = self
            .config_out(DEVICE_SELECT, HARDWARE_MONITOR_DEVICE)
            .and_then(|()| self.config_in(IO_SPACE_LOCK));
        self.port_out(port, EXIT_KEY)?;
        Ok(lock? & IO_SPACE_LOCKED != 0)
    }

    /// Both bytes of every channel's fan count, bank selected once (the caller holds the mutex).
    fn read_counts(&self, base: u16, chip: FanChip) -> Result<Vec<(u8, u8)>, Reason> {
        let index = base + INDEX_OFFSET;
        let data = base + DATA_OFFSET;
        let bank = (chip.channels[0] >> 8) as u8;
        self.port_out(index, BANK_SELECT)?;
        self.port_out(data, bank)?;
        let register = |address: u16| -> Result<u8, Reason> {
            self.port_out(index, (address & 0xFF) as u8)?;
            self.port_in(data)
        };
        chip.channels
            .iter()
            .map(|&address| Ok((register(address)?, register(address + 1)?)))
            .collect()
    }

    fn port_out(&self, port: u16, value: u8) -> Result<(), Reason> {
        self.pawnio.call(
            c"ioctl_pio_outb",
            &[u64::from(port), u64::from(value)],
            &mut [],
        )
    }

    fn port_in(&self, port: u16) -> Result<u8, Reason> {
        let mut out = [0u64];
        self.pawnio
            .call(c"ioctl_pio_inb", &[u64::from(port)], &mut out)?;
        Ok(out[0] as u8)
    }

    fn config_in(&self, register: u8) -> Result<u8, Reason> {
        let mut out = [0u64];
        self.pawnio
            .call(c"ioctl_superio_inb", &[u64::from(register)], &mut out)?;
        Ok(out[0] as u8)
    }

    fn config_in_word(&self, register: u8) -> Result<u16, Reason> {
        let mut out = [0u64];
        self.pawnio
            .call(c"ioctl_superio_inw", &[u64::from(register)], &mut out)?;
        Ok(out[0] as u16)
    }

    fn config_out(&self, register: u8, value: u8) -> Result<(), Reason> {
        self.pawnio.call(
            c"ioctl_superio_outb",
            &[u64::from(register), u64::from(value)],
            &mut [],
        )
    }
}

/// The system-wide mutex monitoring tools hold while talking to the Super I/O chip (research R5).
struct IsaBusMutex(HANDLE);

/// Held while alive; released on drop.
struct IsaBusGuard<'a>(&'a IsaBusMutex);

impl IsaBusMutex {
    fn open() -> Option<IsaBusMutex> {
        // SAFETY: the name is a NUL-terminated literal; create-or-open, not initially owned.
        unsafe { CreateMutexW(None, false, w!(r"Global\Access_ISABUS.HTP.Method")) }
            .ok()
            .map(IsaBusMutex)
    }

    /// Waits up to `MUTEX_WAIT_MS`; an abandoned mutex (its owner died) is ours.
    fn lock(&self) -> Option<IsaBusGuard<'_>> {
        // SAFETY: `self.0` is a valid mutex handle.
        let result = unsafe { WaitForSingleObject(self.0, MUTEX_WAIT_MS) };
        (result == WAIT_OBJECT_0 || result == WAIT_ABANDONED).then_some(IsaBusGuard(self))
    }
}

impl Drop for IsaBusGuard<'_> {
    fn drop(&mut self) {
        // SAFETY: this thread owns the mutex (acquired in `lock`).
        let _ = unsafe { ReleaseMutex(self.0.0) };
    }
}

impl Drop for IsaBusMutex {
    fn drop(&mut self) {
        // SAFETY: the handle is ours and closed exactly once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// The SMBIOS baseboard product name (e.g. "PRIME B840-PLUS WIFI"), for Fan labels (research R6);
/// empty if unknown.
pub fn board_product() -> String {
    let mut buf = [0u16; 128];
    let mut size = size_of_val(&buf) as u32;
    // SAFETY: `buf` and `size` describe a valid writable buffer.
    let err = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!(r"HARDWARE\DESCRIPTION\System\BIOS"),
            w!("BaseBoardProduct"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut c_void),
            Some(&mut size),
        )
    };
    if err.is_err() {
        return String::new();
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len]).trim().to_string()
}
