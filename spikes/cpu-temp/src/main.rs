//! CPU Temperature spike: prints Tctl/Tdie once per second (specs/001-cpu-temp-spike).

#![deny(unsafe_code)]

#[allow(unsafe_code)]
mod platform;
mod reading;

use std::process::ExitCode;
use std::time::{Duration, Instant};

use platform::PawnIo;
use reading::{Outcome, Reason, THM_TCON_CUR_TMP};

const MODULE_FILE: &str = "AMDFamily17.bin";
const TICK: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(50);

fn main() -> ExitCode {
    platform::install_ctrl_handler();

    // Startup checks in data-model order; a failure here is permanent for the run.
    let sensor = startup();

    loop {
        let started = Instant::now();
        let outcome = match &sensor {
            Ok(pawnio) => match pawnio.read_smn(THM_TCON_CUR_TMP) {
                Ok(raw) => reading::classify(reading::decode_tctl(raw)),
                Err(reason) => Outcome::Unavailable(reason),
            },
            Err(reason) => Outcome::Unavailable(*reason),
        };
        println!(
            "{}",
            reading::format_line(&platform::local_clock(), &outcome)
        );

        while started.elapsed() < TICK {
            if platform::stop_requested() {
                return ExitCode::SUCCESS;
            }
            std::thread::sleep(POLL);
        }
    }
}

fn startup() -> Result<PawnIo, Reason> {
    if !platform::is_elevated() {
        return Err(Reason::NotElevated);
    }
    let (vendor, family) = platform::cpu_vendor_family();
    if !reading::is_supported(&vendor, family) {
        return Err(Reason::UnsupportedCpu);
    }
    let module = std::env::current_exe()
        .map_err(|_| Reason::DriverNotFound)?
        .with_file_name(MODULE_FILE);
    PawnIo::open(&module)
}
