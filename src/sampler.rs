//! The sampler thread: reads every Metric once per second and hands an immutable `Snapshot` to
//! the UI thread (research R10). It owns all per-tick state, so no locks are needed.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use crate::domain::history::History;
use crate::domain::metric::{MetricKind, Reading, Reason};
use crate::domain::status_dot;
use crate::domain::warning::{self, Thresholds, WarningLevel};
use crate::platform::cpu::CpuTimes;
use crate::platform::gpu::{GpuReadings, GpuSource};
use crate::platform::pawnio::CpuTemperatureSource;
use crate::platform::{drives, memory};
use crate::snapshot::{APP_NAME, DriveState, MetricState, Snapshot};

const TICK: Duration = Duration::from_secs(1);
const JOIN_LIMIT: Duration = Duration::from_secs(2);
/// How often the Drive list is re-enumerated (FR-022: within 5 s of a change).
const DRIVE_ENUMERATION: Duration = Duration::from_secs(5);

/// All per-tick state and Metric sources. Created on the sampler thread, so sources that are not
/// `Send` (raw driver handles) never cross threads. A source that failed to open yields its
/// `Reason` every tick; each source fails independently (Constitution VIII).
struct Sampler {
    cpu: CpuTimes,
    cpu_temperature: Result<CpuTemperatureSource, Reason>,
    gpu: Result<GpuSource, Reason>,
    drive_letters: Vec<char>,
    drives_enumerated: Option<Instant>,
    /// Previous Warning Levels, for hysteresis (FR-015).
    levels: [Option<WarningLevel>; 6],
    drive_levels: BTreeMap<char, WarningLevel>,
    histories: [History; 6],
    /// Wall-clock time of the previous tick, to detect sleep (FR-021).
    previous_tick: Option<SystemTime>,
}

impl Sampler {
    fn new() -> Sampler {
        Sampler {
            cpu: CpuTimes::new(),
            cpu_temperature: CpuTemperatureSource::open(),
            gpu: GpuSource::open(),
            drive_letters: Vec::new(),
            drives_enumerated: None,
            levels: [None; 6],
            drive_levels: BTreeMap::new(),
            histories: Default::default(),
            previous_tick: None,
        }
    }

    /// One tick. Must never panic (Constitution VIII).
    fn tick(&mut self) -> Snapshot {
        let gpu = match &mut self.gpu {
            Ok(source) => source.read(),
            Err(reason) => GpuReadings::unavailable(*reason),
        };
        let cpu_temperature = match &self.cpu_temperature {
            Ok(source) => source.read(),
            Err(reason) => Reading::Unavailable(*reason),
        };
        let readings = [
            self.cpu.sample(),
            cpu_temperature,
            gpu.usage,
            gpu.temperature,
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
        Snapshot {
            metrics,
            drives,
            tooltip: status_dot::tooltip(APP_NAME, &dot),
            status_dot: dot,
        }
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

/// Stops the sampler thread when `stop()` is called or when dropped.
pub struct SamplerHandle {
    stop: Option<Sender<()>>,
    done: mpsc::Receiver<()>,
    thread: Option<JoinHandle<()>>,
}

/// Starts sampling; `on_snapshot` is called on the sampler thread once per second.
pub fn spawn(on_snapshot: impl Fn(Snapshot) + Send + 'static) -> std::io::Result<SamplerHandle> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
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

/// Ticks until `stop` is signalled or disconnected.
fn run(on_snapshot: &impl Fn(Snapshot), stop: &mpsc::Receiver<()>) {
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
        match stop.recv_timeout(deadline - now) {
            Err(RecvTimeoutError::Timeout) => continue,
            Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

impl SamplerHandle {
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
}
