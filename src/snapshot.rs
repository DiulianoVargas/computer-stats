//! The immutable result of one sampler tick, handed to the UI thread (data-model.md).

use crate::domain::drive::Drive;
use crate::domain::history::History;
use crate::domain::metric::{MetricKind, Reading, Reason};
use crate::domain::status_dot::StatusDot;
use crate::domain::warning::WarningLevel;

#[derive(Debug, Clone, PartialEq)]
pub struct MetricState {
    pub kind: MetricKind,
    pub reading: Reading,
    /// `None` while unavailable: an unavailable Metric has no Warning Level (FR-016).
    pub level: Option<WarningLevel>,
    /// The last 60 seconds, for the Sparkline (FR-020).
    pub history: History,
}

/// One Drive's Disk Space and its Warning Level, or why it could not be read.
#[derive(Debug, Clone, PartialEq)]
pub enum DriveState {
    Ready { drive: Drive, level: WarningLevel },
    Unavailable { letter: char, reason: Reason },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// In Panel order (`MetricKind::ALL`).
    pub metrics: [MetricState; 6],
    /// Sorted by Drive letter.
    pub drives: Vec<DriveState>,
    pub status_dot: StatusDot,
    pub tooltip: String,
}

pub const APP_NAME: &str = "Computer Stats";

impl Snapshot {
    /// Every Metric unavailable for `reason`; used before the first real tick.
    pub fn unavailable(reason: Reason) -> Snapshot {
        Snapshot {
            metrics: MetricKind::ALL.map(|kind| MetricState {
                kind,
                reading: Reading::Unavailable(reason),
                level: None,
                history: History::default(),
            }),
            drives: Vec::new(),
            status_dot: StatusDot {
                level: WarningLevel::Normal,
                cause: None,
            },
            tooltip: APP_NAME.to_string(),
        }
    }
}
