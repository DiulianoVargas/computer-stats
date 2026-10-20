//! Fan Speeds (006 data-model): the supported monitoring chips, fan count → RPM, which motherboard
//! Fans are shown and with what label, and the GPU Fan's "not reported" rule.

use super::history::History;
use super::metric::{MAX_FAN_RPM, MetricKind, Reading, Reason};

/// The two chip families the app reads, which differ in how they are entered and how a fan's
/// registers turn into RPM (research R3, R9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipFamily {
    /// Nuvoton NCT67xx: 13-bit fan counts in bank 4.
    Nuvoton,
    /// ITE IT86xx (Gigabyte boards): 16-bit fan counts, no banks.
    Ite,
}

/// The two registers holding one channel's fan count. For Nuvoton the high byte includes the bank
/// (`0x4B0` = bank 4, register `0xB0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FanRegisters {
    pub high: u16,
    pub low: u16,
}

const fn nuvoton(address: u16) -> FanRegisters {
    FanRegisters {
        high: address,
        low: address + 1,
    }
}

const fn ite(low: u16, high: u16) -> FanRegisters {
    FanRegisters { high, low }
}

/// The NCT6796D–NCT6799D / NCT6701D fan counts, in channel order (research R3).
const NUVOTON_CHANNELS: [FanRegisters; 7] = [
    nuvoton(0x4B0),
    nuvoton(0x4B2),
    nuvoton(0x4B4),
    nuvoton(0x4B6),
    nuvoton(0x4B8),
    nuvoton(0x4BA),
    nuvoton(0x4CC),
];

/// The IT8628E / IT8686E / IT8688E fan counts, in channel order (research R9).
const ITE_CHANNELS: [FanRegisters; 6] = [
    ite(0x0D, 0x18),
    ite(0x0E, 0x19),
    ite(0x0F, 0x1A),
    ite(0x80, 0x81),
    ite(0x82, 0x83),
    ite(0x4C, 0x4D),
];

/// A supported motherboard monitoring chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FanChip {
    /// For diagnostics only.
    pub name: &'static str,
    pub family: ChipFamily,
    /// Fan count registers per channel, in channel order.
    pub channels: &'static [FanRegisters],
}

/// The Nuvoton chip for a Super I/O chip ID and revision (research R3); `None` → not one we read.
pub fn fan_chip(id: u8, revision: u8) -> Option<FanChip> {
    let (name, count) = match (id, revision) {
        (0xD4, 0x23) => ("Nuvoton NCT6796D", 6),
        (0xD4, 0x2A) => ("Nuvoton NCT6796D-R", 7),
        (0xD4, 0x51) => ("Nuvoton NCT6797D", 7),
        (0xD4, 0x2B) => ("Nuvoton NCT6798D", 7),
        (0xD8, 0x02) => ("Nuvoton NCT6799D", 7),
        (0xD8, 0x06) => ("Nuvoton NCT6701D", 7),
        _ => return None,
    };
    Some(FanChip {
        name,
        family: ChipFamily::Nuvoton,
        channels: &NUVOTON_CHANNELS[..count],
    })
}

/// The ITE chip for a 16-bit chip ID (research R9); `None` → not one we read.
pub fn ite_chip(id: u16) -> Option<FanChip> {
    let name = match id {
        0x8628 => "ITE IT8628E",
        0x8686 => "ITE IT8686E",
        0x8688 => "ITE IT8688E",
        _ => return None,
    };
    Some(FanChip {
        name,
        family: ChipFamily::Ite,
        channels: &ITE_CHANNELS,
    })
}

/// ITE channels 4–6 count only when enabled in the fan tachometer mode register (`0x0C`): bit 4
/// for channel 4, bit 5 for channel 5, bit 2 for channel 6. Returns a mask of disabled channels
/// (bit `i` = channel `i + 1`).
pub fn ite_disabled_channels(modes: u8) -> u8 {
    let mut disabled = 0;
    for (channel, bit) in [(3, 4), (4, 5), (5, 2)] {
        if modes & (1 << bit) == 0 {
            disabled |= 1 << channel;
        }
    }
    disabled
}

/// Whether a hardware monitor base address is usable: at least 0x100 and 8-byte aligned within a
/// 4 KiB I/O page (research R2).
pub fn is_valid_base(address: u16) -> bool {
    address >= 0x100 && address & 0xF007 == 0
}

/// One channel's reading in one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FanSample {
    Rpm(f32),
    /// 0 RPM: stopped, or nothing connected.
    Stopped,
    /// A misread: a gap in the Sparkline.
    Invalid,
}

const MAX_COUNT: u32 = 0x1FFF;
const MIN_COUNT: u32 = 0x15;

/// ITE: the 16-bit count at or below this is a misread.
const ITE_MIN_COUNT: u32 = 0x3F;

/// Converts a channel's two count bytes into a sample. Nuvoton: 13-bit count
/// (`high << 5 | low & 0x1F`), RPM = 1 350 000 / count. ITE: 16-bit count, RPM = 1 350 000 /
/// (2 × count).
pub fn fan_speed(family: ChipFamily, high: u8, low: u8) -> FanSample {
    let rpm = match family {
        ChipFamily::Nuvoton => {
            let count = (u32::from(high) << 5) | (u32::from(low) & 0x1F);
            if count >= MAX_COUNT {
                return FanSample::Stopped;
            }
            if count < MIN_COUNT {
                return FanSample::Invalid;
            }
            1_350_000.0 / count as f32
        }
        ChipFamily::Ite => {
            let count = (u32::from(high) << 8) | u32::from(low);
            if count == 0xFFFF {
                return FanSample::Stopped;
            }
            if count <= ITE_MIN_COUNT {
                return FanSample::Invalid;
            }
            1_350_000.0 / (2 * count) as f32
        }
    };
    if rpm > MAX_FAN_RPM {
        FanSample::Invalid
    } else {
        FanSample::Rpm(rpm)
    }
}

/// Header labels of known boards, by SMBIOS baseboard product name, in channel order (research
/// R6). A board is added only once its channel order has been checked on that board.
const KNOWN_BOARDS: &[(&str, &[&str])] = &[];

/// The label a known board gives its CPU cooler's header (spec FR-011).
const CPU_FAN_LABEL: &str = "CPU fan";

pub fn labels_for(board: &str) -> Option<&'static [&'static str]> {
    KNOWN_BOARDS
        .iter()
        .find(|(name, _)| *name == board)
        .map(|(_, labels)| *labels)
}

/// A Fan Speed reading, with the range check every Fan Speed shares (the GPU Fan's rule).
fn fan_reading(rpm: f32) -> Reading {
    Reading::of(MetricKind::GpuFan, rpm)
}

/// A shown motherboard Fan.
#[derive(Debug, Clone, PartialEq)]
pub struct ShownFan {
    pub label: String,
    pub reading: Reading,
    pub history: History,
    /// The board's "CPU fan" header (Desktop Widget, FR-011).
    pub cpu_fan: bool,
}

#[derive(Debug, Clone, Default)]
struct Channel {
    /// Has spun since the app started (FR-003).
    seen: bool,
    reading: Option<Reading>,
    history: History,
}

/// Which motherboard Fans are shown, with their histories (spec FR-003).
#[derive(Debug, Clone, Default)]
pub struct FanList {
    channels: Vec<Channel>,
    labels: Option<&'static [&'static str]>,
}

impl FanList {
    /// `board` is the SMBIOS baseboard product name, for header labels.
    pub fn new(board: &str) -> FanList {
        FanList {
            channels: Vec::new(),
            labels: labels_for(board),
        }
    }

    /// One tick: `gaps` missed seconds (the PC slept), then `samples` (one per channel).
    pub fn update(&mut self, samples: &[FanSample], gaps: usize) {
        if self.channels.len() != samples.len() {
            self.channels.resize_with(samples.len(), Channel::default);
        }
        for (channel, sample) in self.channels.iter_mut().zip(samples) {
            if matches!(sample, FanSample::Rpm(_)) {
                channel.seen = true;
            }
            if !channel.seen {
                continue;
            }
            let reading = match *sample {
                FanSample::Rpm(rpm) => fan_reading(rpm),
                FanSample::Stopped => fan_reading(0.0),
                FanSample::Invalid => Reading::Unavailable(Reason::ReadFailed),
            };
            channel.history.push_gaps(gaps);
            channel.history.push(reading.value());
            channel.reading = Some(reading);
        }
    }

    /// One tick in which no channel could be read (e.g. another tool held the chip).
    pub fn update_failed(&mut self, gaps: usize) {
        for channel in self.channels.iter_mut().filter(|c| c.seen) {
            channel.history.push_gaps(gaps);
            channel.history.push(None);
            channel.reading = Some(Reading::Unavailable(Reason::ReadFailed));
        }
    }

    /// The Fans that have spun since start, in channel order.
    pub fn shown(&self) -> Vec<ShownFan> {
        self.channels
            .iter()
            .enumerate()
            .filter(|(_, c)| c.seen)
            .map(|(i, c)| {
                let label = self
                    .labels
                    .and_then(|labels| labels.get(i))
                    .map_or_else(|| format!("Fan {}", i + 1), |l| (*l).to_string());
                ShownFan {
                    cpu_fan: label == CPU_FAN_LABEL,
                    label,
                    reading: c
                        .reading
                        .unwrap_or(Reading::Unavailable(Reason::ReadFailed)),
                    history: c.history.clone(),
                }
            })
            .collect()
    }
}

/// The GPU driver reports a stopped fan and a missing reading alike as 0, so the GPU Fan shows
/// "not reported" until it has once been above 0 (research R1).
#[derive(Debug, Clone, Copy, Default)]
pub struct GpuFanState {
    reported: bool,
}

impl GpuFanState {
    pub fn next(&mut self, rpm: u32) -> Reading {
        if rpm > 0 {
            self.reported = true;
        }
        if self.reported {
            fan_reading(rpm as f32)
        } else {
            Reading::Unavailable(Reason::NotReported)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_chips() {
        assert_eq!(
            fan_chip(0xD8, 0x06).map(|c| c.name),
            Some("Nuvoton NCT6701D")
        );
        assert_eq!(fan_chip(0xD8, 0x06).map(|c| c.channels.len()), Some(7));
        assert_eq!(fan_chip(0xD4, 0x23).map(|c| c.channels.len()), Some(6));
        assert_eq!(
            fan_chip(0xD8, 0x02).map(|c| c.channels[6]),
            Some(FanRegisters {
                high: 0x4CC,
                low: 0x4CD
            })
        );
        for (id, rev) in [(0xD4, 0x2A), (0xD4, 0x51), (0xD4, 0x2B)] {
            assert!(fan_chip(id, rev).is_some());
        }
    }

    #[test]
    fn unknown_chips_are_unsupported() {
        assert_eq!(fan_chip(0xD5, 0x92), None); // NCT6687D: another layout
        assert_eq!(fan_chip(0xFF, 0xFF), None);
        assert_eq!(fan_chip(0x00, 0x00), None);
    }

    #[test]
    fn ite_chips() {
        let chip = ite_chip(0x8688);
        assert_eq!(chip.map(|c| c.name), Some("ITE IT8688E"));
        assert_eq!(chip.map(|c| c.family), Some(ChipFamily::Ite));
        assert_eq!(chip.map(|c| c.channels.len()), Some(6));
        assert_eq!(
            chip.map(|c| c.channels[3]),
            Some(FanRegisters {
                high: 0x81,
                low: 0x80
            })
        );
        assert!(ite_chip(0x8686).is_some() && ite_chip(0x8628).is_some());
        assert_eq!(ite_chip(0x8689), None); // banked registers: not read yet
        assert_eq!(ite_chip(0xFFFF), None);
    }

    #[test]
    fn ite_count_to_rpm() {
        // count 0x0186 = 390 → 1 350 000 / 780 ≈ 1731 RPM.
        assert!(
            matches!(fan_speed(ChipFamily::Ite, 0x01, 0x86), FanSample::Rpm(r) if (1730.0..1732.0).contains(&r))
        );
        assert_eq!(fan_speed(ChipFamily::Ite, 0xFF, 0xFF), FanSample::Stopped);
        assert_eq!(fan_speed(ChipFamily::Ite, 0x00, 0x3F), FanSample::Invalid);
        // 0x40 = 64 → 10 547 RPM: above the plausible range.
        assert_eq!(fan_speed(ChipFamily::Ite, 0x00, 0x40), FanSample::Invalid);
        // 0x44 = 68 → 9926 RPM.
        assert!(matches!(
            fan_speed(ChipFamily::Ite, 0x00, 0x44),
            FanSample::Rpm(_)
        ));
    }

    #[test]
    fn ite_disabled_channels_follow_the_mode_register() {
        assert_eq!(ite_disabled_channels(0x00), 0b11_1000);
        assert_eq!(ite_disabled_channels(0b11_0100), 0);
        // Channel 4 enabled only.
        assert_eq!(ite_disabled_channels(0b01_0000), 0b11_0000);
    }

    #[test]
    fn base_addresses() {
        assert!(is_valid_base(0x0290));
        assert!(is_valid_base(0x0A00));
        assert!(!is_valid_base(0x0000));
        assert!(!is_valid_base(0x00F8));
        assert!(!is_valid_base(0x0295));
        assert!(!is_valid_base(0xFFFF));
    }

    #[test]
    fn count_to_rpm() {
        // count 1125 = 0x465: high 0x23, low 0x05 → 1200 RPM.
        assert_eq!(
            fan_speed(ChipFamily::Nuvoton, 0x23, 0x05),
            FanSample::Rpm(1200.0)
        );
        // Upper bits of `low` are ignored.
        assert_eq!(
            fan_speed(ChipFamily::Nuvoton, 0x23, 0xE5),
            FanSample::Rpm(1200.0)
        );
    }

    #[test]
    fn count_boundaries() {
        // 0x1FFF and above (all ones, as a locked chip also reads) → stopped.
        assert_eq!(
            fan_speed(ChipFamily::Nuvoton, 0xFF, 0x1F),
            FanSample::Stopped
        );
        // 0x1FFE → the slowest readable speed.
        assert!(
            matches!(fan_speed(ChipFamily::Nuvoton, 0xFF, 0x1E), FanSample::Rpm(r) if (164.0..166.0).contains(&r))
        );
        // Below 0x15 → misread.
        assert_eq!(
            fan_speed(ChipFamily::Nuvoton, 0x00, 0x14),
            FanSample::Invalid
        );
        assert_eq!(
            fan_speed(ChipFamily::Nuvoton, 0x00, 0x00),
            FanSample::Invalid
        );
        // 0x15..0x86 → above 10 000 RPM → misread; 0x87 (135) → 10 000 RPM exactly.
        assert_eq!(
            fan_speed(ChipFamily::Nuvoton, 0x00, 0x15),
            FanSample::Invalid
        );
        assert_eq!(
            fan_speed(ChipFamily::Nuvoton, 0x04, 0x07),
            FanSample::Rpm(10_000.0)
        );
    }

    #[test]
    fn unseen_channels_are_hidden() {
        let mut list = FanList::new("");
        list.update(&[FanSample::Stopped, FanSample::Rpm(900.0)], 0);
        let shown = list.shown();
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].label, "Fan 2");
        assert_eq!(shown[0].reading.value(), Some(900.0));
        assert!(!shown[0].cpu_fan);
    }

    #[test]
    fn a_fan_that_stops_stays_at_zero() {
        let mut list = FanList::new("");
        list.update(&[FanSample::Rpm(1100.0)], 0);
        list.update(&[FanSample::Stopped], 0);
        let shown = list.shown();
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].reading.value(), Some(0.0));
        assert_eq!(
            shown[0].history.iter().collect::<Vec<_>>(),
            vec![Some(1100.0), Some(0.0)]
        );
    }

    #[test]
    fn misreads_and_failed_ticks_are_gaps() {
        let mut list = FanList::new("");
        list.update(&[FanSample::Rpm(1100.0)], 0);
        list.update(&[FanSample::Invalid], 0);
        list.update_failed(0);
        list.update(&[FanSample::Rpm(1000.0)], 2);
        let shown = list.shown();
        assert_eq!(shown[0].reading.value(), Some(1000.0));
        assert_eq!(
            shown[0].history.iter().collect::<Vec<_>>(),
            vec![Some(1100.0), None, None, None, None, Some(1000.0)]
        );
    }

    #[test]
    fn unknown_boards_number_their_fans() {
        assert_eq!(labels_for("PRIME B840-PLUS WIFI"), None);
        let mut list = FanList::new("PRIME B840-PLUS WIFI");
        list.update(&[FanSample::Rpm(800.0), FanSample::Rpm(900.0)], 0);
        let labels: Vec<String> = list.shown().into_iter().map(|f| f.label).collect();
        assert_eq!(labels, ["Fan 1", "Fan 2"]);
    }

    #[test]
    fn gpu_fan_is_not_reported_until_it_spins() {
        let mut state = GpuFanState::default();
        assert_eq!(state.next(0), Reading::Unavailable(Reason::NotReported));
        assert_eq!(state.next(830).value(), Some(830.0));
        assert_eq!(state.next(0).value(), Some(0.0));
    }
}
