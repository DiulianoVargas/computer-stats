//! Computer Stats: a tray visualizer for Metrics (specs/002-tray-metrics-panel).

#![deny(unsafe_code)]

pub mod domain;
#[allow(unsafe_code)]
mod platform;
mod sampler;
mod snapshot;
mod startup;
mod ui;

/// Code generated from `ui/*.slint`; it uses `unwrap` internally, outside our control.
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod generated {
    slint::include_modules!();
}

/// Runs the app until the user chooses Quit.
pub fn run() -> Result<(), String> {
    ui::run()
}
