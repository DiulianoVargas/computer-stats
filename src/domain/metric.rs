//! Metrics, their readings and why a reading can be unavailable (data-model.md).

/// The six per-PC Metrics shown with a Sparkline. Disk Space is per Drive (`drive.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricKind {
    CpuUsage,
    CpuTemperature,
    GpuUsage,
    GpuTemperature,
    Vram,
    MemoryUsage,
}

impl MetricKind {
    /// Every Metric, in Panel order (FR-009).
    pub const ALL: [MetricKind; 6] = [
        MetricKind::CpuUsage,
        MetricKind::CpuTemperature,
        MetricKind::GpuUsage,
        MetricKind::GpuTemperature,
        MetricKind::Vram,
        MetricKind::MemoryUsage,
    ];

    pub fn display_name(self) -> &'static str {
        match self {
            MetricKind::CpuUsage => "CPU Usage",
            MetricKind::CpuTemperature => "CPU Temperature",
            MetricKind::GpuUsage => "GPU Usage",
            MetricKind::GpuTemperature => "GPU Temperature",
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

impl Reading {
    /// A plain value (no byte counts), validated for `kind`.
    pub fn of(kind: MetricKind, value: f32) -> Reading {
        Reading::validated(kind, value, None, None)
    }

    /// Percentages are clamped to 0–100; temperatures outside 0–115 °C (or NaN) become
    /// `Unavailable(ReadFailed)` (rule from spike 001).
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
