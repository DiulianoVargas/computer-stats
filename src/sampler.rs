//! The sampler thread: reads every Metric once per second and hands an immutable `Snapshot` to
//! the UI thread (research R10). It owns all per-tick state, so no locks are needed.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use crate::domain::fan::FanList;
use crate::domain::history::History;
use crate::domain::metric::{MetricKind, Reading, Reason};
use crate::domain::status_dot;
use crate::domain::warning::{self, Thresholds, WarningLevel};
use crate::platform::cpu::CpuTimes;
use crate::platform::gpu::{GpuReadings, GpuSource};
use crate::platform::pawnio::CpuTemperatureSource;
use crate::platform::superio::{self, FanSource};
use crate::platform::{drives, memory};
use crate::process_sampler::ProcessSampler;
use crate::snapshot::{APP_NAME, DriveState, FansState, MetricState, Snapshot};

const METRICS: usize = MetricKind::ALL.len();
const TICK: Duration = Duration::from_secs(1);
const JOIN_LIMIT: Duration = Duration::from_secs(2);
/// How often the Drive list is re-enumerated (FR-022: within 5 s of a change).
const DRIVE_ENUMERATION: Duration = Duration::from_secs(5);
/// How often a lost or missing GPU is opened again. A driver update or reset invalidates the open
/// adapter handle, and the adapter returns with a new LUID.
const GPU_REOPEN: Duration = Duration::from_secs(5);

/// All per-tick state and Metric sources. Created on the sampler thread, so sources that are not
/// `Send` (raw driver handles) never cross threads. A source that failed to open yields its
/// `Reason` every tick; each source fails independently (Constitution VIII).
struct Sampler {
    cpu: CpuTimes,
    cpu_temperature: Result<CpuTemperatureSource, Reason>,
    gpu: Result<GpuSource, Reason>,
    gpu_opened: Instant,
    /// Motherboard Fans (006).
    fans: Result<FanSource, Reason>,
    fan_list: FanList,
    drive_letters: Vec<char>,
    drives_enumerated: Option<Instant>,
    /// Previous Warning Levels, for hysteresis (FR-015).
    levels: [Option<WarningLevel>; METRICS],
    drive_levels: BTreeMap<char, WarningLevel>,
    histories: [History; METRICS],
    /// Wall-clock time of the previous tick, to detect sleep (FR-021).
    previous_tick: Option<SystemTime>,
    /// Present only while the Processes View is visible (003 FR-018).
    processes: Option<ProcessSampler>,
    /// The last Snapshot, reused for an immediate Processes-only tick.
    last: Option<Snapshot>,
}

impl Sampler {
    fn new() -> Sampler {
        Sampler {
            cpu: CpuTimes::new(),
            cpu_temperature: CpuTemperatureSource::open(),
            gpu: GpuSource::open(),
            gpu_opened: Instant::now(),
            fans: FanSource::open(),
            fan_list: FanList::new(&superio::board_product()),
            drive_letters: Vec::new(),
            drives_enumerated: None,
            levels: [None; METRICS],
            drive_levels: BTreeMap::new(),
            histories: Default::default(),
            previous_tick: None,
            processes: None,
            last: None,
        }
    }

    /// Starts or stops Processes sampling. Stopping drops its buffer and caches.
    fn set_processes_visible(&mut self, visible: bool) {
        if visible {
            self.processes.get_or_insert_with(ProcessSampler::new);
        } else {
            self.processes = None;
        }
    }

    /// An extra tick right after the Processes View opens: fresh Processes, the last Metrics
    /// (sampling Metrics twice in one second would skew Sparklines and hysteresis).
    fn processes_only_tick(&mut self) -> Option<Snapshot> {
        let mut snapshot = self.last.clone()?;
        snapshot.processes = self.processes.as_mut().and_then(ProcessSampler::sample);
        Some(snapshot)
    }

    /// One tick. Must never panic (Constitution VIII).
    fn tick(&mut self) -> Snapshot {
        let gpu = match &mut self.gpu {
            Ok(source) => source.read(),
            Err(reason) => GpuReadings::unavailable(*reason),
        };
        if gpu.lost() && self.gpu_opened.elapsed() >= GPU_REOPEN {
            // Close the old handle before enumerating again.
            self.gpu = Err(Reason::ReadFailed);
            self.gpu = GpuSource::open();
            self.gpu_opened = Instant::now();
        }
        let cpu_temperature = match &self.cpu_temperature {
            Ok(source) => source.read(),
            Err(reason) => Reading::Unavailable(*reason),
        };
        let readings = [
            self.cpu.sample(),
            cpu_temperature,
            gpu.usage,
            gpu.temperature,
            gpu.fan,
            gpu.vram,
            memory::memory_usage(),
        ];
        let gaps = self.missed_seconds();
        for kind in MetricKind::ALL {
            let i = kind.index();
            self.histories[i].push_gaps(gaps);
            self.histories[i].push(readings[i].value());
            self.levels[i] = readings[i].value().map(|v| match thresholds(kind) {
                Some(t) => warning::next_level(self.levels[i], v, t),
                None => WarningLevel::Normal,
            });
        }
        let metrics = MetricKind::ALL.map(|kind| MetricState {
            kind,
            reading: readings[kind.index()],
            level: self.levels[kind.index()],
            history: self.histories[kind.index()].clone(),
        });
        let fans = self.read_fans(gaps);
        let drives = self.read_drives();

        let dot = status_dot::compute(
            &metrics.each_ref().map(|m| (m.kind, m.level)),
            &drives
                .iter()
                .map(|d| match d {
                    DriveState::Ready { drive, level } => (drive.letter, Some(*level)),
                    DriveState::Unavailable { letter, .. } => (*letter, None),
                })
                .collect::<Vec<_>>(),
        );
        let snapshot = Snapshot {
            metrics,
            fans,
            drives,
            tooltip: status_dot::tooltip(APP_NAME, &dot),
            status_dot: dot,
            processes: self.processes.as_mut().and_then(ProcessSampler::sample),
        };
        self.last = Some(Snapshot {
            processes: None,
            ..snapshot.clone()
        });
        snapshot
    }

    /// Seconds with no tick since the previous one (the PC slept): wall-clock gaps above 1.5 s
    /// become `min(elapsed − 1, 60)` gap samples (research R10).
    fn missed_seconds(&mut self) -> usize {
        let now = SystemTime::now();
        let missed = self
            .previous_tick
            .and_then(|t| now.duration_since(t).ok())
            .filter(|elapsed| elapsed.as_secs_f32() > 1.5)
            .map_or(0, |elapsed| (elapsed.as_secs_f32() - 1.0).round() as usize);
        self.previous_tick = Some(now);
        missed.min(crate::domain::history::CAPACITY)
    }

    /// Motherboard Fans, after `gaps` missed seconds. A failure leaves the other Metrics alone.
    fn read_fans(&mut self, gaps: usize) -> FansState {
        let source = match &mut self.fans {
            Ok(source) => source,
            Err(reason) => return FansState::Unavailable(*reason),
        };
        match source.read() {
            Ok(samples) => {
                self.fan_list.update(&samples, gaps);
                FansState::Fans(self.fan_list.shown())
            }
            // A Fan already shown keeps its row, with a gap, through a failed read.
            Err(Reason::ReadFailed) if !self.fan_list.shown().is_empty() => {
                self.fan_list.update_failed(gaps);
                FansState::Fans(self.fan_list.shown())
            }
            Err(reason) => FansState::Unavailable(reason),
        }
    }

    /// Free space of known Drives every tick; the list itself every `DRIVE_ENUMERATION`.
    fn read_drives(&mut self) -> Vec<DriveState> {
        let due = self
            .drives_enumerated
            .is_none_or(|at| at.elapsed() >= DRIVE_ENUMERATION);
        if due {
            self.drive_letters = drives::enumerate();
            self.drives_enumerated = Some(Instant::now());
        }
        // Forget levels of Drives that disappeared.
        let letters = &self.drive_letters;
        self.drive_levels
            .retain(|letter, _| letters.contains(letter));

        let mut states = Vec::with_capacity(self.drive_letters.len());
        for &letter in &self.drive_letters {
            states.push(match drives::read(letter) {
                Ok(drive) => {
                    let previous = self.drive_levels.get(&letter).copied();
                    let level = warning::next_level(previous, drive.used_pct(), disk_space());
                    self.drive_levels.insert(letter, level);
                    DriveState::Ready { drive, level }
                }
                Err(reason) => {
                    self.drive_levels.remove(&letter);
                    DriveState::Unavailable { letter, reason }
                }
            });
        }
        states
    }
}

/// FR-014 thresholds. Debug builds can lower them all by `COMPUTER_STATS_TEST_THRESHOLD_OFFSET`
/// (e.g. `60`) to see Warning colors without loading the PC (quickstart Q4–Q6).
fn thresholds(kind: MetricKind) -> Option<Thresholds> {
    warning::thresholds(kind).map(offset)
}

fn disk_space() -> Thresholds {
    offset(warning::DISK_SPACE)
}

#[cfg(debug_assertions)]
fn offset(t: Thresholds) -> Thresholds {
    let by = std::env::var("COMPUTER_STATS_TEST_THRESHOLD_OFFSET")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.0);
    Thresholds {
        warning: t.warning - by,
        critical: t.critical - by,
    }
}

#[cfg(not(debug_assertions))]
fn offset(t: Thresholds) -> Thresholds {
    t
}

/// Messages from the UI thread to the sampler.
enum Command {
    /// Start or stop sampling Processes (the Processes View was shown or hidden).
    ProcessesVisible(bool),
}

/// Stops the sampler thread when `stop()` is called or when dropped (dropping the sender stops it).
pub struct SamplerHandle {
    stop: Option<Sender<Command>>,
    done: mpsc::Receiver<()>,
    thread: Option<JoinHandle<()>>,
}

/// Starts sampling; `on_snapshot` is called on the sampler thread once per second.
pub fn spawn(on_snapshot: impl Fn(Snapshot) + Send + 'static) -> std::io::Result<SamplerHandle> {
    let (stop_tx, stop_rx) = mpsc::channel::<Command>();
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let thread = std::thread::Builder::new()
        .name("sampler".into())
        .spawn(move || {
            run(&on_snapshot, &stop_rx);
            // Sources are dropped (driver handles closed) before we report completion.
            let _ = done_tx.send(());
        })?;
    Ok(SamplerHandle {
        stop: Some(stop_tx),
        done: done_rx,
        thread: Some(thread),
    })
}

/// Ticks until the command channel is disconnected.
fn run(on_snapshot: &impl Fn(Snapshot), commands: &mpsc::Receiver<Command>) {
    let mut sampler = Sampler::new();
    let mut deadline = Instant::now();
    loop {
        on_snapshot(sampler.tick());
        // Next whole-second deadline; if we fell behind (e.g. sleep), restart from now.
        deadline += TICK;
        let now = Instant::now();
        if deadline <= now {
            deadline = now + TICK;
        }
        // Commands may arrive before the deadline; keep waiting for it afterwards.
        loop {
            let wait = deadline.saturating_duration_since(Instant::now());
            match commands.recv_timeout(wait) {
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
                Ok(Command::ProcessesVisible(visible)) => {
                    sampler.set_processes_visible(visible);
                    if visible && let Some(snapshot) = sampler.processes_only_tick() {
                        on_snapshot(snapshot);
                    }
                }
            }
        }
    }
}

impl SamplerHandle {
    /// Starts (immediately, with one extra Snapshot) or stops Processes sampling.
    pub fn set_processes_visible(&self, visible: bool) {
        if let Some(tx) = &self.stop {
            let _ = tx.send(Command::ProcessesVisible(visible));
        }
    }

    /// Signals the thread and waits up to 2 s for it to finish; past that it is left to the OS.
    pub fn stop(&mut self) {
        self.stop.take();
        if let Some(thread) = self.thread.take()
            && self.done.recv_timeout(JOIN_LIMIT).is_ok()
        {
            let _ = thread.join();
        }
    }
}

impl Drop for SamplerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn ticks_immediately_and_stops_promptly() {
        let count = Arc::new(Mutex::new(0));
        let c = Arc::clone(&count);
        let mut handle = spawn(move |_| {
            if let Ok(mut n) = c.lock() {
                *n += 1;
            }
        })
        .ok();
        std::thread::sleep(Duration::from_millis(100));
        let started = Instant::now();
        if let Some(h) = handle.as_mut() {
            h.stop();
        }
        assert!(started.elapsed() < Duration::from_millis(500));
        assert_eq!(count.lock().map(|n| *n).ok(), Some(1));
    }

    #[test]
    fn processes_visible_ticks_at_once_with_processes() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s = Arc::clone(&seen);
        let Ok(mut handle) = spawn(move |snapshot| {
            if let Ok(mut v) = s.lock() {
                v.push(snapshot.processes.map(|p| p.apps.len()));
            }
        }) else {
            return;
        };
        std::thread::sleep(Duration::from_millis(150));
        handle.set_processes_visible(true);
        std::thread::sleep(Duration::from_millis(400));
        handle.stop();
        let seen = seen.lock().map(|v| v.clone()).unwrap_or_default();
        // The first tick had no Processes; the extra one, well before the next second, has them.
        assert_eq!(seen.len(), 2, "{seen:?}");
        assert_eq!(seen[0], None);
        assert!(seen[1].is_some_and(|n| n > 10));
    }
}
