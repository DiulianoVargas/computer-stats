//! Computer Stats: a tray visualizer for Metrics (specs/002-tray-metrics-panel).

#![deny(unsafe_code)]

mod crash_log;
pub mod domain;
#[allow(unsafe_code)]
mod platform;
mod process_sampler;
mod processes_ui;
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
    crash_log::install();
    let result = ui::run();
    if let Err(e) = &result {
        crash_log::write(&format!("startup failed: {e}"));
    }
    result
}
