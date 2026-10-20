# Measurements: Tray Icon and Metrics Panel

Budgets (Constitution I): Panel closed ≤ 30 MB private / < 0.5% CPU; Panel open ≤ 45 MB / < 2% CPU;
release exe ≤ 10 MB. Measured with `scripts/measure.ps1` on the target PC (Ryzen 9 7900, Windows 11).

## Executable size

| Build | Size | Note |
|-------|------|------|
| Empty Slint window, opt-level 3 | 8.2 MiB | Slint winit backend pulls muda, arboard, webbrowser, resvg, ICU (non-optional) |
| Empty Slint window, opt-level "s" | 7.3 MiB | adopted in `Cargo.toml` |
| Empty Slint window, opt-level "z" | 6.9 MiB | kept in reserve if the budget gets tight |

## T017: Skeleton baseline (2026-10-20)

Tray Icon + hidden Panel + sampler with no sources. Run with `__COMPAT_LAYER=RunAsInvoker`
(not elevated; elevation does not change memory or CPU here), `-Minutes 2`, Panel never shown.

| Private memory (max) | CPU (mean) | Growth | Exe |
|----------------------|------------|--------|-----|
| 2.5 MB | 0.00 % | −0.1 MB | 7.38 MB |

The Slint window (winit + softbuffer) is created lazily on the first `show()`, so its cost is not in
this number; it is measured with the Panel open in US1 (T036).

## US1: Panel with real Metrics (2026-10-20)

Not elevated (`__COMPAT_LAYER=RunAsInvoker`), so CPU Temperature (PawnIO) is not loaded. Exe:
8.25 MiB (+0.9 MiB over the skeleton: Panel rows, `ScrollView`).

| Scenario | Private memory (max) | CPU (mean) |
|----------|----------------------|------------|
| Panel open, 2 min | 23.1 MB | 0.03 % |
| Panel closed, with NVML, 2 min | 22.1 MB | 0.02 % |
| Panel closed, without NVML (temporary switch), 2 min | 2.6 MB | 0.00 % |

### T028: NVML footprint

**NVML costs ≈ 19.5 MB private memory** (`nvml.dll` + `nvmlInit_v2`). The Slint window itself costs
only ≈ 1 MB. Spike 001 measured PawnIO at ≈ 7 MB, so the elevated, Panel-closed total is expected at
≈ 29 MB against the 30 MB budget, with no headroom for the App Volumes feature (Core Audio).

The plan's mitigation ("load lazily / unload after N failures") does not apply: the GPU Metrics are
sampled every second for the Status Dot and Sparklines. See plan.md Complexity Tracking.

## After ADR-0003: GPU Metrics through D3DKMT (2026-10-20)

Not elevated; same scenario as above. Exe 8.25 MiB.

| Scenario | Private memory (max) | CPU (mean) |
|----------|----------------------|------------|
| Panel open, 1 min | 3.6 MB | 0.04 % |
| Panel closed, 2 min | 3.5 MB | 0.04 % |

Accuracy vs `nvidia-smi` (GTX 1660 SUPER, idle): VRAM used 1.3 GB vs 1325 MiB and Windows'
`GPU Adapter Memory\Dedicated Usage` counter 1.30 GB; GPU Temperature 37 °C vs 35 °C; GPU Usage 0 %
vs the `GPU Engine` counters' 1.1 % (nvidia-smi reports 10 %: a different definition). VRAM total is
5.8 GB (`DedicatedVideoMemorySize`, usable by Windows) vs 6.0 GB in nvidia-smi.

## US2 + US3 checks (2026-10-20, debug build, not elevated)

- Q4/Q6 with `COMPUTER_STATS_TEST_THRESHOLD_OFFSET=20`: Memory Usage (70 %) and Drive C: (92 % used)
  turned red in the Panel, including the Drive bar. With real thresholds, C: at 92 % used is amber
  (Warning) and the Status Dot stays Normal, as FR-017 requires.
- Q7: a 5–8 s all-core load shows as a bump in the CPU Usage Sparkline; history before the app
  started is left empty (no fabricated values).
- Pending user check: the Status Dot on the Tray Icon and its tooltip; the Sparkline gap after
  sleep/resume.
- Panel compacted after user feedback: 340 × 320 → 320 × 270 logical px with the same content.

## T058: Release budgets, all user stories (2026-10-20)

Release build, not elevated (PawnIO not loaded; spike 001 measured it at ≈ 7 MB, so ≈ 10.5 MB is
expected elevated). Exe **8.28 MiB** (budget 10 MB).

| Scenario | Private memory (max) | CPU (mean) | Growth | Budget |
|----------|----------------------|------------|--------|--------|
| Panel closed, 3 min | 3.6 MB | 0.04 % | +0.7 MB (warm-up from 2.7 MB) | ≤ 30 MB, < 0.5 % |
| Panel open, 20 s (verified open throughout) | 3.5 MB | 0.03 % | −0.1 MB | ≤ 45 MB, < 2 % |

Longer Panel-open runs were cut short by focus changes on the desktop (the Panel correctly closes
when another window is activated). Pending, to run by the user from an elevated terminal: the
same measurement elevated, and the 24 h run for SC-003.

## Polish checks (2026-10-20)

- Q10 single instance: a second launch exited with code 0 and the first instance's Panel opened
  in the foreground.
- Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo deny check`,
  `cargo test` (65 tests) pass; `unsafe` appears only under `src/platform/`; no `unwrap`/`expect`
  outside tests; the only spawned process is System32's `schtasks.exe`.
