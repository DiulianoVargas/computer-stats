//! Start with Windows: a Task Scheduler task managed through `schtasks.exe`, the only process the
//! app may spawn (Constitution IV; research R7). Safe `std::process` only.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::domain::task_xml::task_xml;

const TASK_NAME: &str = "Computer Stats";
/// `CREATE_NO_WINDOW`: no console flashes when spawning schtasks.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Whether the task exists. Checked once at startup; the UI caches it.
pub fn is_enabled() -> bool {
    schtasks(&["/Query", "/TN", TASK_NAME]).is_ok()
}

pub fn enable() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let user = user_id().ok_or("unknown user")?;
    let xml = task_xml(&exe.to_string_lossy(), &user);

    let file = std::env::temp_dir().join("computer-stats-task.xml");
    // UTF-16 LE with BOM, as the XML declaration says.
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
    std::fs::write(&file, bytes).map_err(|e| e.to_string())?;

    let path = file.to_string_lossy().into_owned();
    let result = schtasks(&["/Create", "/TN", TASK_NAME, "/XML", &path, "/F"]);
    let _ = std::fs::remove_file(&file);
    result
}

pub fn disable() -> Result<(), String> {
    schtasks(&["/Delete", "/TN", TASK_NAME, "/F"])
}

/// Runs System32's schtasks (never one found on PATH: we run elevated) and maps a non-zero exit to
/// an error.
fn schtasks(args: &[&str]) -> Result<(), String> {
    let system =
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    let status = Command::new(system.join("System32").join("schtasks.exe"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("schtasks exited with {status}"))
    }
}

/// `DOMAIN\user` of the signed-in user.
fn user_id() -> Option<String> {
    let user = std::env::var("USERNAME").ok()?;
    Some(match std::env::var("USERDOMAIN") {
        Ok(domain) if !domain.is_empty() => format!(r"{domain}\{user}"),
        _ => user,
    })
}
