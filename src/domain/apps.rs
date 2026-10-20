//! Apps in the Processes View: Processes grouped by program file, with CPU % and private memory
//! (003 data-model.md; research R2–R4).

use std::collections::{BTreeMap, HashMap};

/// One Process's identity. A PID alone is not one: Windows reuses PIDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProcessId {
    pub pid: u32,
    pub create_time: u64,
}

/// One Process in one tick.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessSample {
    pub id: ProcessId,
    /// e.g. `chrome.exe`
    pub image_name: String,
    /// Full program file path, when readable.
    pub path: Option<String>,
    /// Kernel + user time, cumulative, in 100 ns units.
    pub cpu_time: u64,
    /// Private working set.
    pub private_bytes: u64,
}

/// An App's identity: its program file (lowercase path), or `name:<image name>` when the path
/// cannot be read.
pub type AppKey = String;

pub fn app_key(sample: &ProcessSample) -> AppKey {
    match &sample.path {
        Some(path) => path.to_lowercase(),
        None => format!("name:{}", sample.image_name.to_lowercase()),
    }
}

/// An App's icon as straight RGBA, `size × size`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconPixels {
    pub size: u32,
    pub rgba: Vec<u8>,
}

/// One row of the Processes View.
#[derive(Debug, Clone, PartialEq)]
pub struct AppUsage {
    pub key: AppKey,
    pub name: String,
    pub path: Option<String>,
    /// At least one.
    pub processes: Vec<ProcessId>,
    /// `None` until at least one of its Processes has two samples.
    pub cpu_pct: Option<f32>,
    pub private_bytes: u64,
}

/// The file name without extension: the fallback App name (`C:\x\Code.exe` → `Code`).
pub fn file_stem(sample: &ProcessSample) -> String {
    let name = sample
        .path
        .as_deref()
        .and_then(|p| p.rsplit(['\\', '/']).next())
        .unwrap_or(&sample.image_name);
    match name.rsplit_once('.') {
        Some((stem, ext)) if ext.eq_ignore_ascii_case("exe") && !stem.is_empty() => stem.into(),
        _ => name.into(),
    }
}

/// Groups Processes into Apps. A Process's CPU % is
/// `Δcpu_time / (Δwall × logical_cpus) × 100`, only when the same `ProcessId` was in `previous`;
/// an App's CPU % is the sum, clamped to 0–100. PID 0 (System Idle Process) is skipped.
pub fn group(
    samples: &[ProcessSample],
    previous: &HashMap<ProcessId, u64>,
    elapsed_100ns: u64,
    logical_cpus: u32,
    name: impl Fn(&AppKey, &ProcessSample) -> String,
) -> Vec<AppUsage> {
    let capacity = elapsed_100ns as f64 * logical_cpus.max(1) as f64;
    let mut apps: BTreeMap<AppKey, AppUsage> = BTreeMap::new();
    for sample in samples.iter().filter(|s| s.id.pid != 0) {
        let key = app_key(sample);
        let cpu = previous
            .get(&sample.id)
            .filter(|_| capacity > 0.0)
            .map(|&before| sample.cpu_time.saturating_sub(before) as f64 / capacity * 100.0);
        let app = apps.entry(key.clone()).or_insert_with(|| AppUsage {
            name: name(&key, sample),
            key,
            path: sample.path.clone(),
            processes: Vec::new(),
            cpu_pct: None,
            private_bytes: 0,
        });
        app.processes.push(sample.id);
        app.private_bytes += sample.private_bytes;
        if let Some(cpu) = cpu {
            let total = app.cpu_pct.unwrap_or(0.0) as f64 + cpu;
            app.cpu_pct = Some(total.min(100.0) as f32);
        }
    }
    apps.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(pid: u32, created: u64, path: Option<&str>, cpu: u64, mem: u64) -> ProcessSample {
        ProcessSample {
            id: ProcessId {
                pid,
                create_time: created,
            },
            image_name: path
                .and_then(|p| p.rsplit('\\').next())
                .unwrap_or("System")
                .to_string(),
            path: path.map(str::to_string),
            cpu_time: cpu,
            private_bytes: mem,
        }
    }

    fn by_stem(_: &AppKey, s: &ProcessSample) -> String {
        file_stem(s)
    }

    // One second on a 4-CPU machine: 4 × 10_000_000 units of capacity.
    const SECOND: u64 = 10_000_000;

    #[test]
    fn groups_by_program_file_case_insensitively() {
        let samples = [
            sample(10, 1, Some(r"C:\B\chrome.exe"), 0, 100),
            sample(11, 1, Some(r"c:\b\CHROME.EXE"), 0, 50),
            sample(12, 1, Some(r"C:\B\code.exe"), 0, 7),
        ];
        let apps = group(&samples, &HashMap::new(), SECOND, 4, by_stem);
        assert_eq!(apps.len(), 2);
        let chrome = apps.iter().find(|a| a.key == r"c:\b\chrome.exe");
        assert_eq!(
            chrome.map(|a| (a.processes.len(), a.private_bytes)),
            Some((2, 150))
        );
    }

    #[test]
    fn unreadable_paths_group_by_image_name_and_idle_is_skipped() {
        let samples = [sample(0, 0, None, 0, 0), sample(4, 0, None, 0, 1)];
        let apps = group(&samples, &HashMap::new(), SECOND, 4, by_stem);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].key, "name:system");
        assert_eq!(apps[0].name, "System");
    }

    #[test]
    fn cpu_is_summed_and_shared_across_cpus() {
        let a = sample(10, 1, Some(r"C:\x.exe"), SECOND, 0);
        let b = sample(11, 1, Some(r"C:\x.exe"), 3 * SECOND, 0);
        let previous = HashMap::from([(a.id, 0), (b.id, SECOND)]);
        // a ran 1 s and b 2 s, out of 4 s of capacity → 75 %.
        let apps = group(&[a, b], &previous, SECOND, 4, by_stem);
        assert_eq!(apps[0].cpu_pct, Some(75.0));
    }

    #[test]
    fn new_processes_have_no_cpu_yet_and_pid_reuse_is_not_inherited() {
        let old = sample(10, 1, Some(r"C:\x.exe"), 0, 0);
        let reused = sample(10, 2, Some(r"C:\x.exe"), 5 * SECOND, 0);
        let previous = HashMap::from([(old.id, 0)]);
        let apps = group(&[reused], &previous, SECOND, 4, by_stem);
        assert_eq!(apps[0].cpu_pct, None);
    }

    #[test]
    fn cpu_is_clamped() {
        let a = sample(10, 1, Some(r"C:\x.exe"), 10 * SECOND, 0);
        let previous = HashMap::from([(a.id, 0)]);
        let apps = group(&[a], &previous, SECOND, 4, by_stem);
        assert_eq!(apps[0].cpu_pct, Some(100.0));
    }

    #[test]
    fn file_stem_fallbacks() {
        assert_eq!(
            file_stem(&sample(1, 0, Some(r"C:\x\Code.exe"), 0, 0)),
            "Code"
        );
        assert_eq!(file_stem(&sample(1, 0, None, 0, 0)), "System");
        let registry = ProcessSample {
            image_name: "Registry".into(),
            ..sample(2, 0, None, 0, 0)
        };
        assert_eq!(file_stem(&registry), "Registry");
    }
}
