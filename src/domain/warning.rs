//! Warning Levels: fixed thresholds with hysteresis (FR-014, FR-015; data-model.md).

use super::metric::MetricKind;

/// How concerning a Metric's value is. Ordered: `Normal < Warning < Critical`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WarningLevel {
    Normal,
    Warning,
    Critical,
}

impl WarningLevel {
    pub fn display_name(self) -> &'static str {
        match self {
            WarningLevel::Normal => "Normal",
            WarningLevel::Warning => "Warning",
            WarningLevel::Critical => "Critical",
        }
    }

    fn lower(self) -> WarningLevel {
        match self {
            WarningLevel::Critical => WarningLevel::Warning,
            _ => WarningLevel::Normal,
        }
    }
}

/// Values at or above `warning` / `critical` (% or °C).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    pub warning: f32,
    pub critical: f32,
}

impl Thresholds {
    fn of(self, level: WarningLevel) -> f32 {
        match level {
            WarningLevel::Critical => self.critical,
            WarningLevel::Warning => self.warning,
            WarningLevel::Normal => f32::NEG_INFINITY,
        }
    }
}

/// Disk Space (% used).
pub const DISK_SPACE: Thresholds = Thresholds {
    warning: 85.0,
    critical: 95.0,
};

/// A level drops only once the value is this far below its threshold (FR-015).
pub const HYSTERESIS: f32 = 2.0;

/// FR-014. CPU Usage and GPU Usage are always Normal.
pub fn thresholds(kind: MetricKind) -> Option<Thresholds> {
    let (warning, critical) = match kind {
        MetricKind::CpuUsage | MetricKind::GpuUsage => return None,
        MetricKind::MemoryUsage => (80.0, 90.0),
        MetricKind::Vram => (85.0, 95.0),
        MetricKind::CpuTemperature => (80.0, 90.0),
        MetricKind::GpuTemperature => (80.0, 87.0),
    };
    Some(Thresholds { warning, critical })
}

/// The new level for value `v`, given the previous level (`None` after an unavailable reading,
/// which starts from Normal):
/// 1. `raw` = the highest level whose threshold `v` meets.
/// 2. If `raw ≥ previous`, the new level is `raw` (rising is immediate).
/// 3. Else, step down one level at a time while `v < threshold(current) − 2`.
pub fn next_level(previous: Option<WarningLevel>, v: f32, t: Thresholds) -> WarningLevel {
    let previous = previous.unwrap_or(WarningLevel::Normal);
    let raw = if v >= t.critical {
        WarningLevel::Critical
    } else if v >= t.warning {
        WarningLevel::Warning
    } else {
        WarningLevel::Normal
    };
    if raw >= previous {
        return raw;
    }
    let mut level = previous;
    while level > raw && v < t.of(level) - HYSTERESIS {
        level = level.lower();
    }
    level
}

#[cfg(test)]
mod tests {
    use super::*;
    use WarningLevel::*;

    fn memory() -> Thresholds {
        thresholds(MetricKind::MemoryUsage).unwrap_or(DISK_SPACE)
    }

    #[test]
    fn data_model_example_sequence() {
        // "Memory: 81 → Warning; 79 → Warning; 77.9 → Normal; 91 → Critical; 89 → Critical;
        //  87.9 → Warning."
        let steps = [
            (81.0, Warning),
            (79.0, Warning),
            (77.9, Normal),
            (91.0, Critical),
            (89.0, Critical),
            (87.9, Warning),
        ];
        let mut level = None;
        for (v, expected) in steps {
            let next = next_level(level, v, memory());
            assert_eq!(next, expected, "value {v}");
            level = Some(next);
        }
    }

    #[test]
    fn exactly_at_threshold_rises() {
        assert_eq!(next_level(Some(Normal), 80.0, memory()), Warning);
        assert_eq!(next_level(Some(Warning), 90.0, memory()), Critical);
    }

    #[test]
    fn exactly_two_below_does_not_drop() {
        assert_eq!(next_level(Some(Warning), 78.0, memory()), Warning);
        assert_eq!(next_level(Some(Critical), 88.0, memory()), Critical);
    }

    #[test]
    fn critical_to_normal_in_one_step_when_far_below() {
        assert_eq!(next_level(Some(Critical), 50.0, memory()), Normal);
    }

    #[test]
    fn unavailable_restarts_from_normal() {
        assert_eq!(next_level(None, 79.0, memory()), Normal);
        assert_eq!(next_level(None, 95.0, memory()), Critical);
    }

    #[test]
    fn fr_014_table() {
        assert_eq!(thresholds(MetricKind::CpuUsage), None);
        assert_eq!(thresholds(MetricKind::GpuUsage), None);
        let gpu_temp = thresholds(MetricKind::GpuTemperature);
        assert_eq!(
            gpu_temp.map(|t| (t.warning, t.critical)),
            Some((80.0, 87.0))
        );
        let vram = thresholds(MetricKind::Vram);
        assert_eq!(vram.map(|t| (t.warning, t.critical)), Some((85.0, 95.0)));
        let cpu_temp = thresholds(MetricKind::CpuTemperature);
        assert_eq!(
            cpu_temp.map(|t| (t.warning, t.critical)),
            Some((80.0, 90.0))
        );
        assert_eq!((DISK_SPACE.warning, DISK_SPACE.critical), (85.0, 95.0));
    }
}
