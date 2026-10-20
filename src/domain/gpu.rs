//! GPU Metrics from Windows' kernel graphics statistics (D3DKMT, as Task Manager; ADR-0003).

/// What we know about a graphics adapter when choosing the GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterCandidate {
    pub render_supported: bool,
    pub software: bool,
    /// The integrated half of a hybrid (laptop) pair.
    pub hybrid_integrated: bool,
    pub dedicated_video_bytes: u64,
}

/// Integrated graphics reserve at most a few hundred MB of system RAM as "dedicated" memory; a
/// dedicated GPU (CONTEXT.md) has at least this much of its own.
pub const MIN_DEDICATED_VIDEO_BYTES: u64 = 1024 * 1024 * 1024;

/// The GPU: the hardware render adapter with the most dedicated video memory, if it has enough
/// to be a dedicated card. Returns its index in `candidates`.
pub fn choose_adapter(candidates: &[AdapterCandidate]) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            c.render_supported
                && !c.software
                && !c.hybrid_integrated
                && c.dedicated_video_bytes >= MIN_DEDICATED_VIDEO_BYTES
        })
        .max_by_key(|(_, c)| c.dedicated_video_bytes)
        .map(|(i, _)| i)
}

/// GPU Usage as Task Manager shows it: the busiest engine (node) over the interval.
/// `previous`/`current` are each node's cumulative running time in 100 ns units, and `elapsed` is
/// the wall-clock interval in the same units. `None` if the inputs cannot be compared.
pub fn usage_pct(previous: &[i64], current: &[i64], elapsed: i64) -> Option<f32> {
    if elapsed <= 0 || previous.len() != current.len() || current.is_empty() {
        return None;
    }
    let busiest = previous
        .iter()
        .zip(current)
        .map(|(a, b)| (b - a).max(0))
        .max()?;
    Some((busiest as f64 / elapsed as f64 * 100.0).min(100.0) as f32)
}

/// GPU Temperature from adapter perf data, reported in tenths of °C; 0 means "not reported".
pub fn temperature_c(deci_celsius: u32) -> Option<f32> {
    (deci_celsius > 0).then(|| deci_celsius as f32 / 10.0)
}

/// One memory segment of the GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    /// Aperture segments are system memory mapped for the GPU, not VRAM.
    pub aperture: bool,
    pub bytes_resident: u64,
}

/// VRAM in use: bytes resident in the GPU's own (non-aperture) segments.
pub fn vram_used(segments: &[Segment]) -> u64 {
    segments
        .iter()
        .filter(|s| !s.aperture)
        .map(|s| s.bytes_resident)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    fn adapter(dedicated: u64) -> AdapterCandidate {
        AdapterCandidate {
            render_supported: true,
            software: false,
            hybrid_integrated: false,
            dedicated_video_bytes: dedicated,
        }
    }

    #[test]
    fn picks_the_dedicated_gpu_over_integrated_graphics() {
        let igpu = adapter(512 * 1024 * 1024);
        let dgpu = adapter(6 * GIB);
        assert_eq!(choose_adapter(&[igpu, dgpu]), Some(1));
        assert_eq!(choose_adapter(&[dgpu, igpu]), Some(0));
    }

    #[test]
    fn integrated_only_is_no_gpu() {
        assert_eq!(choose_adapter(&[adapter(512 * 1024 * 1024)]), None);
        assert_eq!(choose_adapter(&[]), None);
    }

    #[test]
    fn skips_software_display_only_and_hybrid_integrated() {
        let software = AdapterCandidate {
            software: true,
            ..adapter(8 * GIB)
        };
        let display_only = AdapterCandidate {
            render_supported: false,
            ..adapter(8 * GIB)
        };
        let hybrid = AdapterCandidate {
            hybrid_integrated: true,
            ..adapter(2 * GIB)
        };
        assert_eq!(choose_adapter(&[software, display_only, hybrid]), None);
        assert_eq!(
            choose_adapter(&[software, adapter(4 * GIB), hybrid]),
            Some(1)
        );
    }

    #[test]
    fn usage_is_the_busiest_node() {
        // 1 s = 10_000_000 × 100 ns; the 3D node ran 0.25 s, video decode 0.05 s.
        let prev = [1_000, 5_000];
        let cur = [1_000 + 2_500_000, 5_000 + 500_000];
        assert_eq!(usage_pct(&prev, &cur, 10_000_000), Some(25.0));
    }

    #[test]
    fn usage_is_clamped_and_robust() {
        assert_eq!(usage_pct(&[0], &[20_000_000], 10_000_000), Some(100.0));
        // A counter going backwards (adapter reset) counts as idle.
        assert_eq!(usage_pct(&[500], &[100], 10_000_000), Some(0.0));
        assert_eq!(usage_pct(&[0], &[1], 0), None);
        assert_eq!(usage_pct(&[0, 0], &[1], 10), None);
        assert_eq!(usage_pct(&[], &[], 10), None);
    }

    #[test]
    fn temperature_in_tenths() {
        assert_eq!(temperature_c(415), Some(41.5));
        assert_eq!(temperature_c(0), None);
    }

    #[test]
    fn vram_counts_only_local_segments() {
        let segments = [
            Segment {
                aperture: false,
                bytes_resident: 3 * GIB,
            },
            Segment {
                aperture: true,
                bytes_resident: 5 * GIB,
            },
            Segment {
                aperture: false,
                bytes_resident: GIB,
            },
        ];
        assert_eq!(vram_used(&segments), 4 * GIB);
    }
}
