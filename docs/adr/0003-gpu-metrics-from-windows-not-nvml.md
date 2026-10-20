# GPU Metrics from Windows' graphics statistics, not NVML

ADR-0001 planned GPU Usage, VRAM and GPU Temperature through NVIDIA's NVML. Measured in feature 002,
loading `nvml.dll` and calling `nvmlInit_v2` costs ≈ 19.5 MB of private memory, which put the
Panel-closed total at ≈ 29 MB of the 30 MB budget (Constitution I) with PawnIO loaded. We read the
same three Metrics from the Windows kernel graphics interface instead (`D3DKMTEnumAdapters2`,
`D3DKMTQueryStatistics`, `D3DKMTQueryAdapterInfo` in `gdi32.dll`), the source Task Manager uses.
Measured afterwards: 3.5 MB private with the Panel closed.

## Consequences

- No vendor library is loaded; any GPU whose WDDM driver reports statistics works, not only NVIDIA.
- GPU Usage follows Task Manager's definition (the busiest engine), not NVML's. VRAM total is the
  memory Windows can use (e.g. 5.8 GB on a 6 GB card whose firmware reserves the rest).
- GPU Temperature needs a driver that reports adapter perf data (WDDM 2.4+); otherwise it shows
  `— read failed` while the other Metrics keep working.
- The dedicated GPU is chosen as the hardware adapter with the most dedicated video memory, at least
  1 GiB; integrated graphics are ignored.
- Supersedes the NVML part of ADR-0001.
