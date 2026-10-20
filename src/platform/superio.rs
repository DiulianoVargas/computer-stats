//! Motherboard Fan Speeds from the Nuvoton or ITE Super I/O chip through PawnIO's `LpcIO.bin`
//! module (006 research R2–R5, R9). Read-only: the only writes are the chip's access protocol (configuration
//! mode, device / bank / register select) and clearing the firmware's I/O space read lock, as
//! HWiNFO and iCUE do (FR-006, revised). No fan, curve or voltage register is ever written.

use std::ffi::c_void;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};
use windows::core::w;

use super::pawnio::{PawnIo, is_elevated};
use crate::domain::fan::{self, ChipFamily, FanChip, FanSample};
use crate::domain::metric::Reason;

/// How long a tick may wait for another monitoring tool to release the chip (research R5).
const MUTEX_WAIT_MS: u32 = 10;
/// How often a chip that stays locked (or unsupported) is checked again (research R4).
const LOCK_RECHECK: Duration = Duration::from_secs(60);

// Super I/O configuration registers (Nuvoton; ITE where noted).
const CONFIG_PORTS: [u16; 2] = [0x2E, 0x4E];
const ENTER_KEY: u8 = 0x87;
const EXIT_KEY: u8 = 0xAA;
/// ITE: the enter sequence on port 0x2E, and its exit (write 0x02 to configuration control 0x02).
const ITE_ENTER: [u8; 4] = [0x87, 0x01, 0x55, 0x55];
const ITE_CONFIG_CONTROL: u8 = 0x02;
const ITE_EXIT: u8 = 0x02;
const ITE_ENVIRONMENT_DEVICE: u8 = 0x04;
/// ITE hardware monitor: fan tachometer mode (which of channels 4–6 count).
const ITE_FAN_MODES: u8 = 0x0C;
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
    /// `disabled`: channels that never count (bit `i` = channel `i + 1`; ITE only).
    Ready {
        port: u16,
        base: u16,
        chip: FanChip,
        disabled: u8,
    },
    /// Detection is due again at `at`; until then every read gives `reason`.
    Retry { at: Instant, reason: Reason },
}

enum Detection {
    Ready {
        port: u16,
        base: u16,
        chip: FanChip,
        disabled: u8,
    },
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
        let (port, base, chip, disabled) = match self.state {
            State::Ready {
                port,
                base,
                chip,
                disabled,
            } => (port, base, chip, disabled),
            State::Retry { at, reason } => {
                if Instant::now() < at {
                    return Err(reason);
                }
                self.detect_now()?;
                match self.state {
                    State::Ready {
                        port,
                        base,
                        chip,
                        disabled,
                    } => (port, base, chip, disabled),
                    State::Retry { reason, .. } => return Err(reason),
                }
            }
        };

        let Some(_guard) = self.mutex.lock() else {
            // Another tool holds the chip: a gap for every Fan this second.
            return Ok(vec![FanSample::Invalid; chip.channels.len()]);
        };
        let raw = self.read_counts(base, chip)?;
        // Every byte 0xFF on a Nuvoton chip: the firmware locked it again (e.g. after sleep). This
        // second is a gap; the next tick re-runs detection, which clears the lock again.
        if chip.family == ChipFamily::Nuvoton
            && raw.iter().all(|&(high, low)| high == 0xFF && low == 0xFF)
            && self.is_locked(port)?
        {
            self.state = State::Retry {
                at: Instant::now(),
                reason: Reason::LockedByFirmware,
            };
            return Ok(vec![FanSample::Invalid; chip.channels.len()]);
        }
        Ok(raw
            .into_iter()
            .enumerate()
            .map(|(i, (high, low))| {
                if disabled & (1 << i) != 0 {
                    // Not counting: never shown.
                    FanSample::Stopped
                } else {
                    fan::fan_speed(chip.family, high, low)
                }
            })
            .collect())
    }

    /// Runs detection and updates `state`. `Err` only for an unsupported chip (`open` turns it
    /// into a permanent failure).
    fn detect_now(&mut self) -> Result<(), Reason> {
        let mut report = String::new();
        let detection = match self.mutex.lock() {
            Some(_guard) => self.detect(&mut report),
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
            Ok(Detection::Ready {
                port,
                base,
                chip,
                disabled,
            }) => State::Ready {
                port,
                base,
                chip,
                disabled,
            },
            Ok(Detection::Locked) => State::Retry {
                at: Instant::now() + LOCK_RECHECK,
                reason: Reason::LockedByFirmware,
            },
            Ok(Detection::Unsupported) => {
                // What each probe read, so an unknown board can be added (research R10).
                crate::crash_log::write(&format!("fans: unsupported motherboard;{report}"));
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

    /// Finds a supported chip on either configuration port, noting what each probe read in
    /// `report`. The caller holds the ISA bus mutex.
    fn detect(&self, report: &mut String) -> Result<Detection, Reason> {
        for (slot, &port) in CONFIG_PORTS.iter().enumerate() {
            self.pawnio
                .call(c"ioctl_select_slot", &[slot as u64], &mut [])?;
            // Before any enter sequence: a chip another tool left in configuration mode.
            let before = self.chip_id()?;
            self.port_out(port, ENTER_KEY)?;
            self.port_out(port, ENTER_KEY)?;
            let nuvoton = self.chip_id()?;
            report.push_str(&format!(
                " port {port:#04X}: idle {before:#06X}, Nuvoton {nuvoton:#06X}"
            ));
            let found = self.identify(port, report);
            // Always leave configuration mode, whatever was found.
            let exit = self.port_out(port, EXIT_KEY);
            if let Some(detection) = found? {
                exit?;
                return Ok(detection);
            }
            exit?;
            // ITE chips (Gigabyte boards) answer only on 0x2E, to their own enter sequence.
            if port == CONFIG_PORTS[0] {
                for key in ITE_ENTER {
                    self.port_out(port, key)?;
                }
                let ite = self.chip_id()?;
                report.push_str(&format!(", ITE {ite:#06X}"));
                let found = self.identify_ite(port, report);
                let exit = self.config_out(ITE_CONFIG_CONTROL, ITE_EXIT);
                if let Some(detection) = found? {
                    exit?;
                    return self.finish_ite(detection);
                }
                exit?;
            }
        }
        Ok(Detection::Unsupported)
    }

    /// Inside ITE configuration mode: the chip and its environment controller base; `None` when
    /// this port has no supported ITE chip. ITE chips have no read lock.
    fn identify_ite(&self, port: u16, report: &mut String) -> Result<Option<Detection>, Reason> {
        let Some(chip) = fan::ite_chip(self.chip_id()?) else {
            return Ok(None);
        };
        self.pawnio.call(c"ioctl_find_bars", &[], &mut [])?;
        self.config_out(DEVICE_SELECT, ITE_ENVIRONMENT_DEVICE)?;
        let base = self.config_in_word(BASE_ADDRESS)?;
        std::thread::sleep(Duration::from_millis(1));
        let verify = self.config_in_word(BASE_ADDRESS)?;
        if base != verify || !fan::is_valid_base(base) {
            report.push_str(&format!(" ({} base {base:#06X}/{verify:#06X})", chip.name));
            return Ok(Some(Detection::Unsupported));
        }
        Ok(Some(Detection::Ready {
            port,
            base,
            chip,
            disabled: 0,
        }))
    }

    /// After leaving configuration mode: which ITE channels count, read from the hardware monitor
    /// (its I/O range was whitelisted by `find_bars`).
    fn finish_ite(&self, detection: Detection) -> Result<Detection, Reason> {
        let Detection::Ready {
            port, base, chip, ..
        } = detection
        else {
            return Ok(detection);
        };
        let modes = self.monitor_in(base, ITE_FAN_MODES)?;
        Ok(Detection::Ready {
            port,
            base,
            chip,
            disabled: fan::ite_disabled_channels(modes),
        })
    }

    /// Inside configuration mode: the chip and its hardware monitor base, clearing the read lock if
    /// the firmware set it; `None` when this port has no supported chip.
    fn identify(&self, port: u16, report: &mut String) -> Result<Option<Detection>, Reason> {
        let [id, revision] = self.chip_id()?.to_be_bytes();
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
            report.push_str(&format!(" ({} base {base:#06X}/{verify:#06X})", chip.name));
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
        Ok(Some(Detection::Ready {
            port,
            base,
            chip,
            disabled: 0,
        }))
    }

    /// The configuration space's chip ID (`0x20`, high byte) and revision (`0x21`, low byte).
    fn chip_id(&self) -> Result<u16, Reason> {
        let high = self.config_in(CHIP_ID)?;
        let low = self.config_in(CHIP_REVISION)?;
        Ok(u16::from_be_bytes([high, low]))
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

    /// Both bytes of every channel's fan count (the caller holds the mutex). Nuvoton: the bank is
    /// selected once; ITE has no banks.
    fn read_counts(&self, base: u16, chip: FanChip) -> Result<Vec<(u8, u8)>, Reason> {
        if chip.family == ChipFamily::Nuvoton {
            let bank = (chip.channels[0].high >> 8) as u8;
            self.port_out(base + INDEX_OFFSET, BANK_SELECT)?;
            self.port_out(base + DATA_OFFSET, bank)?;
        }
        chip.channels
            .iter()
            .map(|r| {
                let high = self.monitor_in(base, (r.high & 0xFF) as u8)?;
                let low = self.monitor_in(base, (r.low & 0xFF) as u8)?;
                Ok((high, low))
            })
            .collect()
    }

    /// One hardware monitor register (in the current bank).
    fn monitor_in(&self, base: u16, register: u8) -> Result<u8, Reason> {
        self.port_out(base + INDEX_OFFSET, register)?;
        self.port_in(base + DATA_OFFSET)
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
