//! Pure logic: decoding, plausibility, formatting, CPU support. No unsafe, no I/O.

/// Why a CPU Temperature reading is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    NotElevated,
    UnsupportedCpu,
    DriverNotFound,
    ReadFailed,
}

impl Reason {
    pub fn message(&self) -> &'static str {
        match self {
            Reason::NotElevated => "not elevated",
            Reason::UnsupportedCpu => "unsupported CPU",
            Reason::DriverNotFound => "driver not found",
            Reason::ReadFailed => "read failed",
        }
    }
}

/// One Temperature Reading's value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    Celsius(f32),
    Unavailable(Reason),
}

/// Local wall-clock time of a reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    pub h: u8,
    pub m: u8,
    pub s: u8,
}

/// SMN address of THM_TCON_CUR_TMP on AMD family 17h/19h/1Ah.
pub const THM_TCON_CUR_TMP: u32 = 0x0005_9800;

const RANGE_SEL_MASK: u32 = 0x8_0000;
const TJ_SEL_MASK: u32 = 0x3_0000;

const MIN_PLAUSIBLE: f32 = 0.0;
const MAX_PLAUSIBLE: f32 = 115.0;

/// Decodes a raw THM_TCON_CUR_TMP value to Tctl/Tdie in °C (same rules as LibreHardwareMonitor).
pub fn decode_tctl(raw: u32) -> f32 {
    let celsius = (raw >> 21) as f32 * 0.125;
    let offset = raw & RANGE_SEL_MASK != 0 || raw & TJ_SEL_MASK == TJ_SEL_MASK;
    if offset { celsius - 49.0 } else { celsius }
}

/// Rejects physically implausible values (and NaN) as a failed read.
pub fn classify(celsius: f32) -> Outcome {
    if (MIN_PLAUSIBLE..=MAX_PLAUSIBLE).contains(&celsius) {
        Outcome::Celsius(celsius)
    } else {
        Outcome::Unavailable(Reason::ReadFailed)
    }
}

/// One output line, per contracts/cli-output.md.
pub fn format_line(clock: &Clock, outcome: &Outcome) -> String {
    let Clock { h, m, s } = clock;
    match outcome {
        Outcome::Celsius(c) => format!("{h:02}:{m:02}:{s:02}  {c:.1} °C"),
        Outcome::Unavailable(r) => format!("{h:02}:{m:02}:{s:02}  —  ({})", r.message()),
    }
}

/// Whether CPU Temperature can be read through the AMDFamily17 module.
pub fn is_supported(vendor: &[u8; 12], family: u32) -> bool {
    vendor == b"AuthenticAMD" && matches!(family, 0x17 | 0x19 | 0x1A)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: Clock = Clock { h: 14, m: 2, s: 7 };

    // --- T006: decode_tctl -------------------------------------------------

    #[test]
    fn decode_plain_value() {
        assert_eq!(decode_tctl(432 << 21), 54.0);
    }

    #[test]
    fn decode_eighth_degree_resolution() {
        assert_eq!(decode_tctl(433 << 21), 54.125);
    }

    #[test]
    fn decode_range_select_applies_minus_49() {
        assert_eq!(decode_tctl((824 << 21) | 0x80000), 54.0);
    }

    #[test]
    fn decode_tj_select_applies_minus_49() {
        assert_eq!(decode_tctl((824 << 21) | 0x30000), 54.0);
    }

    #[test]
    fn decode_partial_tj_select_has_no_offset() {
        assert_eq!(decode_tctl((824 << 21) | 0x10000), 103.0);
    }

    // --- T007: format_line, values -----------------------------------------

    #[test]
    fn format_value_one_decimal() {
        assert_eq!(
            format_line(&AT, &Outcome::Celsius(54.26)),
            "14:02:07  54.3 °C"
        );
    }

    #[test]
    fn format_pads_clock_and_keeps_trailing_zero() {
        let clock = Clock { h: 9, m: 5, s: 0 };
        assert_eq!(
            format_line(&clock, &Outcome::Celsius(54.0)),
            "09:05:00  54.0 °C"
        );
    }

    // --- T014: classify ----------------------------------------------------

    #[test]
    fn classify_bounds() {
        assert_eq!(classify(-0.1), Outcome::Unavailable(Reason::ReadFailed));
        assert_eq!(classify(0.0), Outcome::Celsius(0.0));
        assert_eq!(classify(115.0), Outcome::Celsius(115.0));
        assert_eq!(classify(115.1), Outcome::Unavailable(Reason::ReadFailed));
    }

    #[test]
    fn classify_rejects_nan() {
        assert_eq!(classify(f32::NAN), Outcome::Unavailable(Reason::ReadFailed));
    }

    // --- T017: format_line, failures ---------------------------------------

    #[test]
    fn format_each_reason() {
        let cases = [
            (Reason::NotElevated, "14:02:07  —  (not elevated)"),
            (Reason::DriverNotFound, "14:02:07  —  (driver not found)"),
            (Reason::UnsupportedCpu, "14:02:07  —  (unsupported CPU)"),
            (Reason::ReadFailed, "14:02:07  —  (read failed)"),
        ];
        for (reason, expected) in cases {
            assert_eq!(format_line(&AT, &Outcome::Unavailable(reason)), expected);
        }
    }

    // --- T018: is_supported ------------------------------------------------

    #[test]
    fn supported_amd_families() {
        for family in [0x17, 0x19, 0x1A] {
            assert!(is_supported(b"AuthenticAMD", family), "family {family:#x}");
        }
    }

    #[test]
    fn unsupported_cpus() {
        assert!(!is_supported(b"AuthenticAMD", 0x15));
        assert!(!is_supported(b"GenuineIntel", 0x06));
    }
}
