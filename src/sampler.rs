//! The sampler thread: reads every Metric once per second and hands an immutable `Snapshot` to
//! the UI thread (research R10). It owns all per-tick state, so no locks are needed.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::domain::metric::Reason;
use crate::snapshot::Snapshot;

const TICK: Duration = Duration::from_secs(1);
const JOIN_LIMIT: Duration = Duration::from_secs(2);

/// All per-tick state and Metric sources. Created on the sampler thread, so sources that are not
/// `Send` (raw driver handles) never cross threads.
struct Sampler {}

impl Sampler {
    fn new() -> Sampler {
        Sampler {}
    }

    /// One tick. Must never panic (Constitution VIII).
    fn tick(&mut self) -> Snapshot {
        Snapshot::unavailable(Reason::ReadFailed)
    }
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
