//! The immutable result of one sampler tick, handed to the UI thread (data-model.md).

use crate::domain::metric::{MetricKind, Reading, Reason};

#[derive(Debug, Clone, PartialEq)]
pub struct MetricState {
    pub kind: MetricKind,
    pub reading: Reading,
}

/// One Drive's Disk Space, or why it could not be read. Completed in US1 (T020).
#[allow(dead_code)] // constructed once Drives are sampled (T027)
#[derive(Debug, Clone, PartialEq)]
pub enum DriveState {
    Unavailable { letter: char, reason: Reason },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// In Panel order (`MetricKind::ALL`).
    pub metrics: [MetricState; 6],
    /// Sorted by Drive letter.
    pub drives: Vec<DriveState>,
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
            }),
            drives: Vec::new(),
            tooltip: APP_NAME.to_string(),
        }
    }
}
