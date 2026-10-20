//! CPU Temperature decoding: AMD family 17h/19h/1Ah (from spike 001) and Intel digital thermal
//! sensors (feature 004).

use super::metric::{MetricKind, Reading, Reason};

/// Which PawnIO module and decoding a CPU needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuKind {
    /// Ryzen and later: `AMDFamily17.bin`, SMN register THM_TCON_CUR_TMP.
    Amd,
    /// `IntelMSR.bin`; `package` when the CPU has a package thermal sensor (CPUID.06H:EAX[6]),
    /// otherwise the first core's sensor is used.
    Intel { package: bool },
}

/// The CPU kind from CPUID: vendor string, effective family, and leaf 6's digital thermal
/// sensor (`has_dts`, EAX[0]) and package thermal management (`has_ptm`, EAX[6]) bits.
pub fn cpu_kind(vendor: &[u8; 12], family: u32, has_dts: bool, has_ptm: bool) -> Option<CpuKind> {
    match vendor {
        b"AuthenticAMD" if matches!(family, 0x17 | 0x19 | 0x1A) => Some(CpuKind::Amd),
        b"GenuineIntel" if has_dts => Some(CpuKind::Intel { package: has_ptm }),
        _ => None,
    }
}

/// `IA32_TEMPERATURE_TARGET`.
pub const MSR_TEMPERATURE_TARGET: u32 = 0x1A2;
/// `IA32_PACKAGE_THERM_STATUS`.
pub const MSR_PACKAGE_THERM_STATUS: u32 = 0x1B1;
/// `IA32_THERM_STATUS` (per core).
pub const MSR_THERM_STATUS: u32 = 0x19C;

const READING_VALID: u64 = 1 << 31;

/// TjMax: `IA32_TEMPERATURE_TARGET` bits 23:16, or 100 °C when the CPU reports none.
pub fn intel_tjmax(temperature_target: u64) -> f32 {
    match (temperature_target >> 16) & 0xFF {
        0 => 100.0,
        t => t as f32,
    }
}

/// Distance below TjMax in a thermal status MSR (bits 22:16).
fn below_tjmax(therm_status: u64) -> f32 {
    ((therm_status >> 16) & 0x7F) as f32
}

/// Package temperature from `IA32_PACKAGE_THERM_STATUS`.
pub fn intel_package(therm_status: u64, tjmax: f32) -> Reading {
    Reading::of(
        MetricKind::CpuTemperature,
        tjmax - below_tjmax(therm_status),
    )
}

/// Core temperature from `IA32_THERM_STATUS`; only valid when its "reading valid" bit is set.
pub fn intel_core(therm_status: u64, tjmax: f32) -> Reading {
    if therm_status & READING_VALID == 0 {
        return Reading::Unavailable(Reason::ReadFailed);
    }
    Reading::of(
        MetricKind::CpuTemperature,
        tjmax - below_tjmax(therm_status),
    )
}

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

#[cfg(test)]
mod tests {
    use super::*;

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
            assert_eq!(
                cpu_kind(b"AuthenticAMD", family, false, false),
                Some(CpuKind::Amd),
                "family {family:#x}"
            );
        }
    }

    #[test]
    fn intel_needs_a_thermal_sensor() {
        assert_eq!(
            cpu_kind(b"GenuineIntel", 6, true, true),
            Some(CpuKind::Intel { package: true })
        );
        assert_eq!(
            cpu_kind(b"GenuineIntel", 6, true, false),
            Some(CpuKind::Intel { package: false })
        );
        assert_eq!(cpu_kind(b"GenuineIntel", 6, false, true), None);
    }

    #[test]
    fn unsupported_cpus() {
        assert_eq!(cpu_kind(b"AuthenticAMD", 0x15, true, true), None);
        assert_eq!(cpu_kind(b"CentaurHauls", 6, true, true), None);
    }

    #[test]
    fn intel_tjmax_from_temperature_target() {
        // TjMax 100 in bits 23:16, with an offset in 29:24 that is ignored.
        assert_eq!(intel_tjmax((5 << 24) | (100 << 16)), 100.0);
        assert_eq!(intel_tjmax(95 << 16), 95.0);
        assert_eq!(intel_tjmax(0), 100.0);
    }

    #[test]
    fn intel_package_temperature() {
        // 42 °C below TjMax 100 → 58 °C; other status bits are ignored.
        let status = (42 << 16) | 0b1011;
        assert_eq!(intel_package(status, 100.0).value(), Some(58.0));
    }

    #[test]
    fn intel_core_requires_valid_bit() {
        let valid = READING_VALID | (30 << 16);
        assert_eq!(intel_core(valid, 95.0).value(), Some(65.0));
        assert_eq!(
            intel_core(30 << 16, 95.0),
            Reading::Unavailable(Reason::ReadFailed)
        );
    }

    #[test]
    fn intel_implausible_is_read_failed() {
        // TjMax 100, 127 below → −27 °C.
        assert_eq!(
            intel_package(0x7F << 16, 100.0),
            Reading::Unavailable(Reason::ReadFailed)
        );
    }
}
