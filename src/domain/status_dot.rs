//! The Status Dot: the worst Warning Level across Metrics, Disk Space only at Critical (FR-017).

use super::metric::MetricKind;
use super::warning::WarningLevel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusDot {
    pub level: WarningLevel,
    /// Display name of the Metric (or `Disk C:`) that caused a level above Normal.
    pub cause: Option<String>,
}

/// Max over every Metric's level, plus each Drive's level only if Critical. Ties go to the first
/// in Panel order (Metrics, then Drives by letter).
pub fn compute(
    metrics: &[(MetricKind, Option<WarningLevel>)],
    drives: &[(char, Option<WarningLevel>)],
) -> StatusDot {
    let metric_levels = metrics
        .iter()
        .filter_map(|(kind, level)| level.map(|l| (l, kind.display_name().to_string())));
    let drive_levels = drives
        .iter()
        .filter(|(_, level)| *level == Some(WarningLevel::Critical))
        .map(|(letter, _)| (WarningLevel::Critical, format!("Disk {letter}:")));

    let mut dot = StatusDot {
        level: WarningLevel::Normal,
        cause: None,
    };
    for (level, name) in metric_levels.chain(drive_levels) {
        if level > dot.level {
            dot = StatusDot {
                level,
                cause: Some(name),
            };
        }
    }
    dot
}

/// `"Computer Stats"`, or `"Computer Stats — <cause> <Warning|Critical>"` (FR-006).
pub fn tooltip(app_name: &str, dot: &StatusDot) -> String {
    match &dot.cause {
        Some(cause) if dot.level > WarningLevel::Normal => {
            format!("{app_name} — {cause} {}", dot.level.display_name())
        }
        _ => app_name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use MetricKind::*;
    use WarningLevel::*;

    fn all_normal() -> Vec<(MetricKind, Option<WarningLevel>)> {
        MetricKind::ALL.iter().map(|&k| (k, Some(Normal))).collect()
    }

    fn set(
        metrics: &mut [(MetricKind, Option<WarningLevel>)],
        kind: MetricKind,
        level: WarningLevel,
    ) {
        if let Some(m) = metrics.iter_mut().find(|(k, _)| *k == kind) {
            m.1 = Some(level);
        }
    }

    #[test]
    fn all_normal_has_no_cause() {
        let dot = compute(&all_normal(), &[('C', Some(Normal))]);
        assert_eq!(dot.level, Normal);
        assert_eq!(dot.cause, None);
        assert_eq!(tooltip("Computer Stats", &dot), "Computer Stats");
    }

    #[test]
    fn warning_plus_critical_is_critical() {
        let mut m = all_normal();
        set(&mut m, MemoryUsage, Warning);
        set(&mut m, CpuTemperature, Critical);
        let dot = compute(&m, &[]);
        assert_eq!(dot.level, Critical);
        assert_eq!(
            tooltip("Computer Stats", &dot),
            "Computer Stats — CPU Temperature Critical"
        );
    }

    #[test]
    fn drive_at_warning_does_not_count() {
        let dot = compute(&all_normal(), &[('C', Some(Warning))]);
        assert_eq!(dot.level, Normal);
    }

    #[test]
    fn drive_at_critical_counts() {
        let dot = compute(&all_normal(), &[('C', Some(Normal)), ('D', Some(Critical))]);
        assert_eq!(dot.level, Critical);
        assert_eq!(
            tooltip("Computer Stats", &dot),
            "Computer Stats — Disk D: Critical"
        );
    }

    #[test]
    fn ties_go_to_panel_order() {
        let mut m = all_normal();
        set(&mut m, MemoryUsage, Warning);
        set(&mut m, Vram, Warning);
        assert_eq!(compute(&m, &[]).cause.as_deref(), Some("VRAM"));

        set(&mut m, MemoryUsage, Critical);
        let dot = compute(&m, &[('C', Some(Critical))]);
        assert_eq!(dot.cause.as_deref(), Some("Memory Usage"));
    }

    #[test]
    fn unavailable_metrics_are_ignored() {
        let mut m = all_normal();
        m[1].1 = None;
        assert_eq!(compute(&m, &[('C', None)]).level, Normal);
    }
}
