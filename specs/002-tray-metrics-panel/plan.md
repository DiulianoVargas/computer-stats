# Implementation Plan: Tray Icon and Metrics Panel

**Branch**: `002-tray-metrics-panel` | **Date**: 2026-10-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/002-tray-metrics-panel/spec.md`

## Summary

The first real app crate, at the repository root. A `sampler` thread reads every Metric once per
second (Win32 system APIs, NVML, and PawnIO from spike 001), keeps the Sparkline histories and
Warning Levels, and sends an immutable Snapshot to the UI thread. The UI thread owns a hand-made
Win32 Tray Icon (with a code-drawn Status Dot) and a single hidden Slint Panel, rendered in
software, that is shown anchored to the icon and hidden when it loses focus. Start with Windows is
a scheduled task managed through `schtasks`. All decision logic (thresholds, hysteresis, Status
Dot, formatting, anchoring, icon pixels, task XML) is pure and unit-tested; all `unsafe` lives in
`src/platform/`.

## Technical Context

**Language/Version**: Rust stable (toolchain 1.99), edition 2024

**Primary Dependencies**: `slint` 1.18 (software renderer, winit backend), `windows` 0.62 (Win32
FFI), `raw-window-handle` 0.6 (Panel HWND); build: `slint-build` 1.18. Runtime: PawnIO +
`AMDFamily17.bin`, `nvml.dll` (both optional)

**Storage**: None. Start with Windows state lives in Task Scheduler.

**Testing**: `cargo test` for pure modules; manual validation per quickstart.md (Q1–Q15);
`scripts/measure.ps1` for budgets

**Target Platform**: Windows 11 x64; target PC AMD Ryzen 9 7900 + NVIDIA GPU

**Project Type**: Desktop tray app (single binary)

**Performance Goals**: Panel visible ≤ 200 ms after click (SC-001); one sample per second

**Constraints**: ≤ 30 MB / < 0.5% CPU with Panel closed, ≤ 45 MB / < 2% CPU open, exe ≤ 10 MB
(Constitution I); never panics on the sampling path (VIII)

**Scale/Scope**: ~2,000 lines of Rust + ~300 lines of `.slint`; 6 Metrics + up to ~10 Drives

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How |
|-----------|--------|-----|
| I. Resource Budgets | ✅ | Software renderer, Panel hidden not re-created, no Panel updates while hidden, PawnIO opened once; `scripts/measure.ps1` added and run in Polish. NVML's footprint is measured early (R9 risk) |
| II. Native Only | ✅ | Slint native window; no web view; `deny.toml` bans `wry`, `webview2-com`, `tauri` |
| III. Unsafe Isolation | ✅ | All FFI in `src/platform/` (`#[allow(unsafe_code)] mod platform;`); crate root `#![deny(unsafe_code)]` |
| IV. Elevated Minimal Surface | ✅ | No network crates (banned); only `schtasks.exe` is spawned; only the signed PawnIO module and System32 `nvml.dll` are loaded |
| V. Software Rendering | ✅ | `renderer-software`, `default-features = false` |
| VI. Dependency Discipline | ✅ | Three runtime crates, each justified (R1, R4); tray and NVML hand-written instead of `tray-icon`/`nvml-wrapper` (R2, R9); `cargo deny check`; Slint royalty-free attribution in `README.md` |
| VII. Sampling Discipline | ✅ | Timer-driven `sampler` thread (R10); fixed 60-slot ring buffers |
| VIII. Graceful Degradation | ✅ | Each source returns `Reading::Unavailable(Reason)`; no `unwrap`/`expect` outside tests (clippy-denied) |
| IX. Code Hygiene | ✅ | fmt, clippy `-D warnings`; identifiers use `CONTEXT.md` terms (`WarningLevel`, `StatusDot`, `Sparkline`, `Drive`, `Panel`, `TrayIcon`) |

**Post-design re-check**: still passing. No new domain terms are introduced; `Snapshot`,
`Sampler`, `History` and `Reason` are internal technical names, not domain vocabulary.

## Project Structure

### Documentation (this feature)

```text
specs/002-tray-metrics-panel/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/ui.md
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
Cargo.toml               # computer-stats; release profile per constitution; clippy unwrap/expect deny
deny.toml                # bans (web views, network), licenses, advisories, duplicates
build.rs                 # slint-build; embed app.manifest via linker args
app.manifest             # highestAvailable, PerMonitorV2, Common Controls v6
README.md                # build/run notes + Slint attribution
scripts/measure.ps1      # budget measurement (Constitution I)
ui/
├── theme.slint          # color tokens (light/dark), Warning colors, spacing
├── sparkline.slint      # Sparkline component (Path from commands string)
├── metric-row.slint     # label + value + Sparkline; unavailable state
├── drive-row.slint      # Drive label + bar + free text
└── panel.slint          # PanelWindow: stacked sections, Esc handling
src/
├── main.rs              # #![deny(unsafe_code)], #![windows_subsystem = "windows"]; startup wiring
├── domain/              # pure, no I/O
│   ├── mod.rs
│   ├── metric.rs        # MetricKind, Reading, Reason
│   ├── warning.rs       # WarningLevel, thresholds, hysteresis
│   ├── status_dot.rs    # StatusDot rule + tooltip text
│   ├── history.rs       # 60-slot ring buffer with gaps
│   ├── drive.rs         # Drive + used %
│   ├── format.rs        # value text (%, °C, GB/TB, "— reason")
│   ├── sparkline.rs     # Path commands string
│   ├── anchor.rs        # Panel placement
│   ├── tray_art.rs      # Tray Icon pixels with Status Dot
│   ├── cpu_temp.rs      # decode_tctl, classify, is_supported (from spike reading.rs)
│   └── task_xml.rs      # Start with Windows task XML
├── sampler.rs           # thread, tick loop, gaps, Snapshot building
├── snapshot.rs          # Snapshot, MetricState, DriveState
├── ui.rs                # Snapshot → Slint properties; Panel show/hide/toggle; Tray Menu actions
├── startup.rs           # Start with Windows via schtasks (safe std::process)
└── platform/            # the only unsafe code
    ├── mod.rs
    ├── cpu.rs           # GetSystemTimes
    ├── memory.rs        # GlobalMemoryStatusEx
    ├── drives.rs        # enumeration, USB exclusion, free space, label
    ├── pawnio.rs        # from spike platform.rs (elevation, CPUID, PawnIo)
    ├── nvml.rs          # runtime-loaded NVML
    ├── tray.rs          # hidden window, Shell_NotifyIconW, menu, TaskbarCreated, HICON
    ├── window.rs        # Panel HWND: tool window, corners, deactivate subclass, foreground
    ├── shell.rs         # icon rect, taskbar edge, work area, monitor DPI
    ├── theme.rs         # SystemUsesLightTheme
    └── instance.rs      # single-instance mutex + OpenPanel message
```

**Structure Decision**: The app is one crate at the repository root. Pure modules in `src/domain/`
hold everything that can be tested without Windows; `src/platform/` is the only place with
`unsafe`. `spikes/cpu-temp/` stays as reference; its code is copied (not linked) into
`domain/cpu_temp.rs` and `platform/pawnio.rs`.

## Implementation Order and Risks

1. **Skeleton + measurement first**: an empty Slint window + Tray Icon, measured with
   `measure.ps1`, before adding Metrics. This tells us early how much of the 30 MB budget Slint +
   winit consume.
2. **NVML footprint** (R9) is measured as soon as GPU Metrics work. Mitigation if too large: load
   NVML lazily and unload after N failures, or document the deviation in Complexity Tracking.
3. **Focus/toggle race** (R4) is the most likely UX bug; it has its own quickstart step (Q1).

## Complexity Tracking

No violations.
