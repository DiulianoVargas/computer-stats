//! Sparkline path commands for Slint's `Path` (research R11): a 60 × 20 viewbox, newest sample on
//! the right, one `M … L …` run per contiguous block so gaps break the line.

use std::fmt::Write;

use super::history::{CAPACITY, History};
use super::metric::MetricKind;

pub const WIDTH: f32 = CAPACITY as f32;
pub const HEIGHT: f32 = 20.0;

/// The vertical scale for Fan Speeds, motherboard Fans included (006 contracts/ui.md).
pub const FAN_SCALE: (f32, f32) = (0.0, 3000.0);

/// The vertical scale: 0–100 for percentages, 20–100 °C for temperatures, 0–3000 RPM for Fans.
pub fn scale(kind: MetricKind) -> (f32, f32) {
    if kind.is_temperature() {
        (20.0, 100.0)
    } else if kind.is_fan() {
        FAN_SCALE
    } else {
        (0.0, 100.0)
    }
}

pub fn path(history: &History, min: f32, max: f32) -> String {
    let mut out = String::new();
    let offset = CAPACITY - history.len();
    let mut in_run = false;
    let mut run_start = 0.0;
    for (i, sample) in history.iter().enumerate() {
        let Some(v) = sample else {
            close_single(&mut out, in_run, run_start);
            in_run = false;
            continue;
        };
        let x = (offset + i) as f32 + 0.5;
        let y = HEIGHT - (v.clamp(min, max) - min) / (max - min) * HEIGHT;
        let command = if in_run { 'L' } else { 'M' };
        if !in_run {
            run_start = x;
        }
        let _ = write!(out, "{}{command} {} {}", sep(&out), fmt(x), fmt(y));
        in_run = true;
    }
    close_single(&mut out, in_run, run_start);
    out
}

/// A run of one sample would draw nothing; extend it into a short horizontal tick.
fn close_single(out: &mut String, in_run: bool, run_start: f32) {
    if in_run && last_command_is_move(out) {
        let _ = write!(out, " h {}", fmt(if run_start > 59.0 { -0.5 } else { 0.5 }));
    }
}

fn last_command_is_move(out: &str) -> bool {
    out.rfind(['M', 'L', 'h'])
        .is_some_and(|i| out[i..].starts_with('M'))
}

fn sep(out: &str) -> &'static str {
    if out.is_empty() { "" } else { " " }
}

/// Compact numbers: at most two decimals, no trailing zeros.
fn fmt(v: f32) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(samples: &[Option<f32>]) -> History {
        let mut h = History::default();
        for &s in samples {
            h.push(s);
        }
        h
    }

    #[test]
    fn empty_history_is_empty_path() {
        assert_eq!(path(&History::default(), 0.0, 100.0), "");
    }

    #[test]
    fn newest_sample_is_at_the_right() {
        let p = path(&history(&[Some(0.0), Some(100.0)]), 0.0, 100.0);
        assert_eq!(p, "M 58.5 20 L 59.5 0");
    }

    #[test]
    fn gaps_break_the_line() {
        let p = path(
            &history(&[Some(50.0), Some(50.0), None, Some(25.0), Some(75.0)]),
            0.0,
            100.0,
        );
        assert_eq!(p, "M 55.5 10 L 56.5 10 M 58.5 15 L 59.5 5");
    }

    #[test]
    fn values_are_clamped_to_the_scale() {
        let (min, max) = scale(MetricKind::CpuTemperature);
        let p = path(&history(&[Some(10.0), Some(120.0)]), min, max);
        assert_eq!(p, "M 58.5 20 L 59.5 0");
    }

    #[test]
    fn fans_use_a_fixed_rpm_scale() {
        assert_eq!(scale(MetricKind::GpuFan), (0.0, 3000.0));
        let (min, max) = scale(MetricKind::GpuFan);
        let p = path(&history(&[Some(1500.0), Some(4000.0)]), min, max);
        assert_eq!(p, "M 58.5 10 L 59.5 0");
    }

    #[test]
    fn isolated_sample_becomes_a_tick() {
        let p = path(
            &history(&[Some(50.0), None, Some(50.0), Some(50.0)]),
            0.0,
            100.0,
        );
        assert_eq!(p, "M 56.5 10 h 0.5 M 58.5 10 L 59.5 10");
        // The newest sample alone ticks leftward to stay inside the viewbox.
        let p = path(&history(&[None, Some(50.0)]), 0.0, 100.0);
        assert_eq!(p, "M 59.5 10 h -0.5");
    }
}
