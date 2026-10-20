//! Per-tick state of the Processes View, owned by the sampler thread and alive only while the View
//! is visible (003 FR-018; research R1, R4–R6).

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::domain::apps::{self, AppKey, AppUsage, IconPixels, ProcessId};
use crate::platform::{icons, processes};

/// Time per tick spent resolving new names and icons (≈ 10 ms each), so the list appears at once
/// and fills in over the next ticks (SC-001).
const RESOLVE_BUDGET: Duration = Duration::from_millis(80);

/// One tick of the Processes View.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessesSample {
    /// Unordered; the UI orders them.
    pub apps: Vec<AppUsage>,
    /// Icons resolved this tick; each key is sent at most once while the View stays open.
    pub icons: Vec<(AppKey, IconPixels)>,
    /// Computer Stats itself: Ending it means Quit (FR-017).
    pub own_pid: u32,
}

pub struct ProcessSampler {
    enumerator: processes::Enumerator,
    logical_cpus: u32,
    previous_cpu: HashMap<ProcessId, u64>,
    previous_at: Option<Instant>,
    /// Program path per Process; `None` once known to be unreadable.
    paths: HashMap<ProcessId, Option<String>>,
    /// Resolved display names per App.
    names: HashMap<AppKey, String>,
    /// Apps whose icon was already sent (or has none).
    icons_done: HashSet<AppKey>,
}

impl ProcessSampler {
    pub fn new() -> ProcessSampler {
        ProcessSampler {
            enumerator: processes::Enumerator::default(),
            logical_cpus: processes::logical_cpus(),
            previous_cpu: HashMap::new(),
            previous_at: None,
            paths: HashMap::new(),
            names: HashMap::new(),
            icons_done: HashSet::new(),
        }
    }

    /// `None` if Windows refused the enumeration this tick.
    pub fn sample(&mut self) -> Option<ProcessesSample> {
        let now = Instant::now();
        let mut samples = self.enumerator.sample()?;

        // Paths only for Processes not seen before; forget exited ones.
        let alive: HashSet<ProcessId> = samples.iter().map(|s| s.id).collect();
        self.paths.retain(|id, _| alive.contains(id));
        for s in samples.iter_mut().filter(|s| s.id.pid != 0) {
            s.path = self
                .paths
                .entry(s.id)
                .or_insert_with(|| processes::image_path(s.id.pid))
                .clone();
        }

        let elapsed = self
            .previous_at
            .map_or(0, |at| (now - at).as_nanos() as u64 / 100);
        let names = &self.names;
        let mut apps = apps::group(
            &samples,
            &self.previous_cpu,
            elapsed,
            self.logical_cpus,
            |key, sample| {
                names
                    .get(key)
                    .cloned()
                    .unwrap_or_else(|| apps::file_stem(sample))
            },
        );
        self.previous_cpu = samples.iter().map(|s| (s.id, s.cpu_time)).collect();
        self.previous_at = Some(now);

        let icons = self.resolve_new(&mut apps, now);
        let running: HashSet<&AppKey> = apps.iter().map(|a| &a.key).collect();
        self.names.retain(|k, _| running.contains(k));
        self.icons_done.retain(|k| running.contains(k));

        Some(ProcessesSample {
            apps,
            icons,
            own_pid: std::process::id(),
        })
    }

    /// Names and icons for Apps not resolved yet, largest memory first, within the time budget.
    fn resolve_new(
        &mut self,
        apps: &mut [AppUsage],
        started: Instant,
    ) -> Vec<(AppKey, IconPixels)> {
        let mut pending: Vec<usize> = (0..apps.len())
            .filter(|&i| apps[i].path.is_some() && !self.icons_done.contains(&apps[i].key))
            .collect();
        pending.sort_by_key(|&i| std::cmp::Reverse(apps[i].private_bytes));

        let mut icons = Vec::new();
        for i in pending {
            if started.elapsed() >= RESOLVE_BUDGET {
                break;
            }
            let app = &mut apps[i];
            let Some(path) = app.path.as_deref() else {
                continue;
            };
            if let Some(name) = icons::file_description(path) {
                app.name = name.clone();
                self.names.insert(app.key.clone(), name);
            }
            if let Some(icon) = icons::small_icon(path) {
                icons.push((app.key.clone(), icon));
            }
            self.icons_done.insert(app.key.clone());
        }
        icons
    }
}
