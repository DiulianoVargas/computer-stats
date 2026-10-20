//! Crash and startup-error log. The app has no console and `panic = "abort"` prints nothing, so
//! failures are appended to `%LOCALAPPDATA%\Computer Stats\crash.log` (Constitution VIII: when
//! something does fail, leave a trace the user can send).

use std::io::Write;
use std::path::PathBuf;

/// Installs the panic hook; call first thing at startup.
pub fn install() {
    std::panic::set_hook(Box::new(|info| {
        let thread = std::thread::current();
        let location = info
            .location()
            .map_or_else(|| "unknown location".to_string(), |l| l.to_string());
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(non-string panic payload)".into());
        write(&format!(
            "panic on thread '{}' at {location}: {message}",
            thread.name().unwrap_or("unnamed")
        ));
    }));
}

/// Appends one line (with a timestamp and the app version) to the log; never fails loudly.
pub fn write(line: &str) {
    let Some(path) = path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(
            file,
            "[unix {seconds}] computer-stats {}: {line}",
            env!("CARGO_PKG_VERSION")
        );
    }
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("Computer Stats").join("crash.log"))
}
