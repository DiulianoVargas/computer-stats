//! GPU Usage, GPU Temperature, GPU Fan and VRAM from the Windows kernel graphics statistics (D3DKMT in
//! `gdi32.dll`, the source Task Manager uses; ADR-0003). No vendor library is loaded.

use std::ffi::c_void;
use std::time::Instant;

use windows::Wdk::Graphics::Direct3D::{
    D3DKMT_ADAPTER_PERFDATA, D3DKMT_ADAPTERINFO, D3DKMT_ADAPTERTYPE, D3DKMT_CLOSEADAPTER,
    D3DKMT_ENUMADAPTERS2, D3DKMT_QUERYADAPTERINFO, D3DKMT_QUERYSTATISTICS,
    D3DKMT_QUERYSTATISTICS_ADAPTER, D3DKMT_QUERYSTATISTICS_NODE, D3DKMT_QUERYSTATISTICS_SEGMENT,
    D3DKMT_SEGMENTSIZEINFO, D3DKMTCloseAdapter, D3DKMTEnumAdapters2, D3DKMTQueryAdapterInfo,
    D3DKMTQueryStatistics, KMTQAITYPE_ADAPTERPERFDATA, KMTQAITYPE_ADAPTERTYPE,
    KMTQAITYPE_GETSEGMENTSIZE, KMTQUERYADAPTERINFOTYPE,
};
use windows::Win32::Foundation::LUID;

use crate::domain::fan;
use crate::domain::gpu::{self, AdapterCandidate, Segment};
use crate::domain::metric::{MetricKind, Reading, Reason};

/// The four GPU Metrics of one tick.
pub struct GpuReadings {
    pub usage: Reading,
    pub temperature: Reading,
    pub fan: Reading,
    pub vram: Reading,
}

impl GpuReadings {
    pub fn unavailable(reason: Reason) -> GpuReadings {
        GpuReadings {
            usage: Reading::Unavailable(reason),
            temperature: Reading::Unavailable(reason),
            fan: Reading::Unavailable(reason),
            vram: Reading::Unavailable(reason),
        }
    }

    /// Usage and VRAM both failed: the adapter is gone (driver updated or reset) or never opened.
    /// Temperature and Fan are left out, since some drivers never report them.
    pub fn lost(&self) -> bool {
        matches!(self.usage, Reading::Unavailable(_))
            && matches!(self.vram, Reading::Unavailable(_))
    }
}

/// An open handle to the GPU. Closed on drop.
pub struct GpuSource {
    adapter: u32,
    luid: LUID,
    node_count: u32,
    segment_count: u32,
    vram_total: u64,
    /// Each node's running time at `sampled_at`, for the next usage delta.
    running: Vec<i64>,
    sampled_at: Instant,
}

impl GpuSource {
    /// Finds the dedicated GPU among the system's adapters (FR: integrated graphics are ignored).
    pub fn open() -> Result<GpuSource, Reason> {
        let adapters = enumerate_adapters().ok_or(Reason::DriverNotFound)?;
        let candidates: Vec<AdapterCandidate> =
            adapters.iter().map(|a| candidate(a.hAdapter)).collect();
        let chosen = gpu::choose_adapter(&candidates);

        // Keep the chosen handle, close every other.
        for (i, a) in adapters.iter().enumerate() {
            if Some(i) != chosen {
                close(a.hAdapter);
            }
        }
        let index = chosen.ok_or(Reason::NoGpu)?;
        let info = adapters[index];

        let mut stats = statistics(info.AdapterLuid, D3DKMT_QUERYSTATISTICS_ADAPTER);
        let ok = query_statistics(&mut stats);
        if !ok {
            close(info.hAdapter);
            return Err(Reason::ReadFailed);
        }
        // SAFETY: the ADAPTER query fills `AdapterInformation`.
        let adapter_info = unsafe { stats.QueryResult.AdapterInformation };

        let mut source = GpuSource {
            adapter: info.hAdapter,
            luid: info.AdapterLuid,
            node_count: adapter_info.NodeCount,
            segment_count: adapter_info.NbSegments,
            vram_total: candidates[index].dedicated_video_bytes,
            running: Vec::new(),
            sampled_at: Instant::now(),
        };
        source.running = source.running_times().unwrap_or_default();
        Ok(source)
    }

    /// Each Metric fails independently with `ReadFailed`.
    pub fn read(&mut self) -> GpuReadings {
        let failed = Reading::Unavailable(Reason::ReadFailed);

        let now = Instant::now();
        let usage = match self.running_times() {
            Some(running) => {
                let elapsed = (now - self.sampled_at).as_nanos() as i64 / 100;
                let pct = gpu::usage_pct(&self.running, &running, elapsed);
                self.running = running;
                self.sampled_at = now;
                pct.map_or(failed, |v| Reading::of(MetricKind::GpuUsage, v))
            }
            None => failed,
        };

        // One perf data query serves GPU Temperature and GPU Fan (006 research R1).
        let perf = self.perf_data();
        let temperature = perf
            .and_then(|p| gpu::temperature_c(p.Temperature))
            .map_or(failed, |v| Reading::of(MetricKind::GpuTemperature, v));
        let fan = perf.map_or(failed, |p| fan::gpu_fan(p.FanRPM));

        let vram = match self.segments() {
            Some(segments) if self.vram_total > 0 => {
                let used = gpu::vram_used(&segments).min(self.vram_total);
                let pct = (used as f64 / self.vram_total as f64 * 100.0) as f32;
                Reading::validated(MetricKind::Vram, pct, Some(used), Some(self.vram_total))
            }
            _ => failed,
        };

        GpuReadings {
            usage,
            temperature,
            fan,
            vram,
        }
    }

    fn running_times(&self) -> Option<Vec<i64>> {
        (0..self.node_count)
            .map(|node| {
                let mut stats = statistics(self.luid, D3DKMT_QUERYSTATISTICS_NODE);
                stats.Anonymous.QueryNode.NodeId = node;
                if !query_statistics(&mut stats) {
                    return None;
                }
                // SAFETY: the NODE query filled `NodeInformation`.
                let node = unsafe { stats.QueryResult.NodeInformation };
                Some(node.GlobalInformation.RunningTime)
            })
            .collect()
    }

    fn segments(&self) -> Option<Vec<Segment>> {
        (0..self.segment_count)
            .map(|segment| {
                let mut stats = statistics(self.luid, D3DKMT_QUERYSTATISTICS_SEGMENT);
                stats.Anonymous.QuerySegment.SegmentId = segment;
                query_statistics(&mut stats).then(|| {
                    // SAFETY: the SEGMENT query fills `SegmentInformation`.
                    let info = unsafe { stats.QueryResult.SegmentInformation };
                    Segment {
                        aperture: info.Aperture != 0,
                        bytes_resident: info.BytesResident,
                    }
                })
            })
            .collect()
    }

    fn perf_data(&self) -> Option<D3DKMT_ADAPTER_PERFDATA> {
        // Input: physical adapter 0 (the default of a zeroed struct).
        query_adapter_info(self.adapter, KMTQAITYPE_ADAPTERPERFDATA)
    }
}

impl Drop for GpuSource {
    fn drop(&mut self) {
        close(self.adapter);
    }
}

// --- D3DKMT helpers ------------------------------------------------------------------------

fn enumerate_adapters() -> Option<Vec<D3DKMT_ADAPTERINFO>> {
    let mut query = D3DKMT_ENUMADAPTERS2::default();
    // SAFETY: a null `pAdapters` asks only for the count.
    if unsafe { D3DKMTEnumAdapters2(&mut query) }.is_err() {
        return None;
    }
    let mut adapters = vec![D3DKMT_ADAPTERINFO::default(); query.NumAdapters as usize];
    query.pAdapters = adapters.as_mut_ptr();
    // SAFETY: `pAdapters` has room for `NumAdapters` entries; the call opens a handle for each,
    // which the caller closes.
    if unsafe { D3DKMTEnumAdapters2(&mut query) }.is_err() {
        return None;
    }
    adapters.truncate(query.NumAdapters as usize);
    Some(adapters)
}

fn candidate(adapter: u32) -> AdapterCandidate {
    let kind = query_adapter_info::<D3DKMT_ADAPTERTYPE>(adapter, KMTQAITYPE_ADAPTERTYPE)
        // SAFETY: `Value` views the same 32 bits as the flags bitfield.
        .map(|t| unsafe { t.Anonymous.Value })
        .unwrap_or(0);
    let sizes = query_adapter_info::<D3DKMT_SEGMENTSIZEINFO>(adapter, KMTQAITYPE_GETSEGMENTSIZE)
        .unwrap_or_default();
    // D3DKMT_ADAPTERTYPE bits: 0 RenderSupported, 2 SoftwareDevice, 5 HybridIntegrated.
    AdapterCandidate {
        render_supported: kind & (1 << 0) != 0,
        software: kind & (1 << 2) != 0,
        hybrid_integrated: kind & (1 << 5) != 0,
        dedicated_video_bytes: sizes.DedicatedVideoMemorySize,
    }
}

/// Queries one fixed-size adapter info struct. `T` must be the struct `kind` returns.
fn query_adapter_info<T: Default>(adapter: u32, kind: KMTQUERYADAPTERINFOTYPE) -> Option<T> {
    let mut data = T::default();
    let mut query = D3DKMT_QUERYADAPTERINFO {
        hAdapter: adapter,
        Type: kind,
        pPrivateDriverData: &mut data as *mut T as *mut c_void,
        PrivateDriverDataSize: size_of::<T>() as u32,
    };
    // SAFETY: the buffer is a `T` of exactly the size passed, matching `kind` (callers).
    unsafe { D3DKMTQueryAdapterInfo(&mut query) }
        .is_ok()
        .then_some(data)
}

fn statistics(
    luid: LUID,
    kind: windows::Wdk::Graphics::Direct3D::D3DKMT_QUERYSTATISTICS_TYPE,
) -> D3DKMT_QUERYSTATISTICS {
    D3DKMT_QUERYSTATISTICS {
        Type: kind,
        AdapterLuid: luid,
        ..Default::default()
    }
}

fn query_statistics(stats: &mut D3DKMT_QUERYSTATISTICS) -> bool {
    // SAFETY: `stats` is a fully initialized query; the result is written in place.
    unsafe { D3DKMTQueryStatistics(stats) }.is_ok()
}

fn close(adapter: u32) {
    let close = D3DKMT_CLOSEADAPTER { hAdapter: adapter };
    // SAFETY: `adapter` is a handle from D3DKMTEnumAdapters2, closed exactly once.
    let _ = unsafe { D3DKMTCloseAdapter(&close) };
}
