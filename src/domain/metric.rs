//! Metrics, their readings and why a reading can be unavailable (data-model.md).

/// The seven per-PC Metrics shown with a Sparkline. Disk Space is per Drive (`drive.rs`);
/// motherboard Fans are per header (`fan.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricKind {
    CpuUsage,
    CpuTemperature,
    GpuUsage,
    GpuTemperature,
    GpuFan,
    Vram,
    MemoryUsage,
}

impl MetricKind {
    /// Every Metric, in Panel order (FR-009).
    pub const ALL: [MetricKind; 7] = [
        MetricKind::CpuUsage,
        MetricKind::CpuTemperature,
        MetricKind::GpuUsage,
        MetricKind::GpuTemperature,
        MetricKind::GpuFan,
        MetricKind::Vram,
        MetricKind::MemoryUsage,
    ];

    pub fn display_name(self) -> &'static str {
        match self {
            MetricKind::CpuUsage => "CPU Usage",
            MetricKind::CpuTemperature => "CPU Temperature",
            MetricKind::GpuUsage => "GPU Usage",
            MetricKind::GpuTemperature => "GPU Temperature",
            MetricKind::GpuFan => "GPU Fan",
            MetricKind::Vram => "VRAM",
            MetricKind::MemoryUsage => "Memory Usage",
        }
    }

    pub fn is_temperature(self) -> bool {
        matches!(
            self,
            MetricKind::CpuTemperature | MetricKind::GpuTemperature
        )
    }

    /// A Fan Speed in RPM (006): always Normal, never clamped to a percentage.
    pub fn is_fan(self) -> bool {
        self == MetricKind::GpuFan
    }

    /// Position in `ALL`.
    pub fn index(self) -> usize {
        self as usize
    }
}

/// Why a Metric or Drive is unavailable; `message()` is shown after `—` in the Panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    NotElevated,
    UnsupportedCpu,
    DriverNotFound,
    ModuleNotFound,
    NoGpu,
    ReadFailed,
    /// The motherboard's monitoring chip is not one the app knows (006 research R3).
    UnsupportedMotherboard,
    /// The firmware locked the monitoring chip against reads and clearing the lock failed (006 R4).
    LockedByFirmware,
}

impl Reason {
    pub fn message(self) -> &'static str {
        match self {
            Reason::NotElevated => "not elevated",
            Reason::UnsupportedCpu => "unsupported CPU",
            Reason::DriverNotFound => "driver not found",
            Reason::ModuleNotFound => "module not found",
            Reason::NoGpu => "no GPU",
            Reason::ReadFailed => "read failed",
            Reason::UnsupportedMotherboard => "unsupported motherboard",
            Reason::LockedByFirmware => "locked by firmware",
        }
    }
}

/// One sample of one Metric.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reading {
    /// `used_bytes`/`total_bytes` are set only for Memory Usage and VRAM.
    Value {
        value: f32,
        used_bytes: Option<u64>,
        total_bytes: Option<u64>,
    },
    Unavailable(Reason),
}

const MIN_TEMPERATURE: f32 = 0.0;
const MAX_TEMPERATURE: f32 = 115.0;
/// Above this a Fan Speed is a misread (006 data-model).
pub const MAX_FAN_RPM: f32 = 10_000.0;

impl Reading {
    /// A plain value (no byte counts), validated for `kind`.
    pub fn of(kind: MetricKind, value: f32) -> Reading {
        Reading::validated(kind, value, None, None)
    }

    /// Percentages are clamped to 0–100; temperatures outside 0–115 °C and Fan Speeds outside
    /// 0–10 000 RPM (or NaN) become `Unavailable(ReadFailed)` (rule from spike 001).
    pub fn validated(
        kind: MetricKind,
        value: f32,
        used_bytes: Option<u64>,
        total_bytes: Option<u64>,
    ) -> Reading {
        if value.is_nan() {
            return Reading::Unavailable(Reason::ReadFailed);
        }
        let value = if kind.is_temperature() {
            if !(MIN_TEMPERATURE..=MAX_TEMPERATURE).contains(&value) {
                return Reading::Unavailable(Reason::ReadFailed);
            }
            value
        } else if kind.is_fan() {
            if !(0.0..=MAX_FAN_RPM).contains(&value) {
                return Reading::Unavailable(Reason::ReadFailed);
            }
            value
        } else {
            value.clamp(0.0, 100.0)
        };
        Reading::Value {
            value,
            used_bytes,
            total_bytes,
        }
    }

    /// The numeric value, or `None` when unavailable.
    pub fn value(&self) -> Option<f32> {
        match self {
            Reading::Value { value, .. } => Some(*value),
            Reading::Unavailable(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_order_matches_index() {
        for (i, kind) in MetricKind::ALL.iter().enumerate() {
            assert_eq!(kind.index(), i);
        }
    }

    #[test]
    fn gpu_fan_follows_gpu_temperature() {
        assert_eq!(MetricKind::ALL.len(), 7);
        assert_eq!(
            MetricKind::GpuFan.index(),
            MetricKind::GpuTemperature.index() + 1
        );
    }

    #[test]
    fn fan_speeds_are_range_checked_not_clamped() {
        assert_eq!(
            Reading::of(MetricKind::GpuFan, 1240.0).value(),
            Some(1240.0)
        );
        assert_eq!(Reading::of(MetricKind::GpuFan, 0.0).value(), Some(0.0));
        assert_eq!(
            Reading::of(MetricKind::GpuFan, 10_001.0),
            Reading::Unavailable(Reason::ReadFailed)
        );
        assert_eq!(
            Reading::of(MetricKind::GpuFan, -1.0),
            Reading::Unavailable(Reason::ReadFailed)
        );
    }

    #[test]
    fn percentages_are_clamped() {
        assert_eq!(
            Reading::of(MetricKind::CpuUsage, 104.0).value(),
            Some(100.0)
        );
        assert_eq!(
            Reading::of(MetricKind::MemoryUsage, -3.0).value(),
            Some(0.0)
        );
        assert_eq!(Reading::of(MetricKind::Vram, 42.5).value(), Some(42.5));
    }

    #[test]
    fn temperature_bounds() {
        let t = MetricKind::CpuTemperature;
        assert_eq!(Reading::of(t, 0.0).value(), Some(0.0));
        assert_eq!(Reading::of(t, 115.0).value(), Some(115.0));
        assert_eq!(
            Reading::of(t, -0.1),
            Reading::Unavailable(Reason::ReadFailed)
        );
        assert_eq!(
            Reading::of(MetricKind::GpuTemperature, 115.1),
            Reading::Unavailable(Reason::ReadFailed)
        );
    }

    #[test]
    fn nan_is_read_failed() {
        assert_eq!(
            Reading::of(MetricKind::CpuUsage, f32::NAN),
            Reading::Unavailable(Reason::ReadFailed)
        );
    }

    #[test]
    fn bytes_are_kept() {
        let r = Reading::validated(MetricKind::MemoryUsage, 50.0, Some(8), Some(16));
        assert_eq!(
            r,
            Reading::Value {
                value: 50.0,
                used_bytes: Some(8),
                total_bytes: Some(16)
            }
        );
    }

    #[test]
    fn reason_messages() {
        let cases = [
            (Reason::NotElevated, "not elevated"),
            (Reason::UnsupportedCpu, "unsupported CPU"),
            (Reason::DriverNotFound, "driver not found"),
            (Reason::ModuleNotFound, "module not found"),
            (Reason::NoGpu, "no GPU"),
            (Reason::ReadFailed, "read failed"),
        ];
        for (reason, text) in cases {
            assert_eq!(reason.message(), text);
        }
    }
}
