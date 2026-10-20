//! Entry point; see `lib.rs`.

#![deny(unsafe_code)]
#![windows_subsystem = "windows"]

use std::process::ExitCode;

fn main() -> ExitCode {
    // No console (windows subsystem): a startup failure can only be reported by the exit code.
    match computer_stats::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
