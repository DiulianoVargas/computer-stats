//! CPU Temperature decoding for AMD family 17h/19h/1Ah, from spike 001 (`reading.rs`).

use super::metric::{MetricKind, Reading};

/// SMN address of THM_TCON_CUR_TMP on AMD family 17h/19h/1Ah.
pub const THM_TCON_CUR_TMP: u32 = 0x0005_9800;

const RANGE_SEL_MASK: u32 = 0x8_0000;
const TJ_SEL_MASK: u32 = 0x3_0000;

/// Decodes a raw THM_TCON_CUR_TMP value to Tctl/Tdie in °C (same rules as LibreHardwareMonitor).
pub fn decode_tctl(raw: u32) -> f32 {
    let celsius = (raw >> 21) as f32 * 0.125;
    let offset = raw & RANGE_SEL_MASK != 0 || raw & TJ_SEL_MASK == TJ_SEL_MASK;
    if offset { celsius - 49.0 } else { celsius }
}

/// A raw register value as a validated CPU Temperature reading (implausible → read failed).
pub fn classify(raw: u32) -> Reading {
    Reading::of(MetricKind::CpuTemperature, decode_tctl(raw))
}

/// Whether CPU Temperature can be read through the AMDFamily17 module.
pub fn is_supported(vendor: &[u8; 12], family: u32) -> bool {
    vendor == b"AuthenticAMD" && matches!(family, 0x17 | 0x19 | 0x1A)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::metric::Reason;

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

    #[test]
    fn classify_bounds() {
        assert_eq!(classify(432 << 21).value(), Some(54.0));
        // 921 × 0.125 = 115.125 °C: implausible.
        assert_eq!(
            classify(921 << 21),
            Reading::Unavailable(Reason::ReadFailed)
        );
        // Offset below zero: −49 °C.
        assert_eq!(classify(0x80000), Reading::Unavailable(Reason::ReadFailed));
    }

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
