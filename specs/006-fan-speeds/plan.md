# Implementation Plan: Fan Speeds

**Branch**: `006-fan-speeds` | **Date**: 2026-10-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/006-fan-speeds/spec.md`

## Summary

Two independent sources feed the existing per-second Snapshot:

- **GPU Fan** (US2): the `FanRPM` field of the D3DKMT perf data `platform::gpu` already reads for
  GPU Temperature. It becomes a seventh `MetricKind`, shown in the GPU group of the Panel and the
  Desktop Widget. Spike S2 confirmed it on the developer's GTX 1660 SUPER (830 RPM).
- **Motherboard fans** (US1): the Nuvoton Super I/O chip through PawnIO's `LpcIO.bin`, read on the
  sampler thread under the system-wide ISA bus mutex shared with HWiNFO and other tools. A new
  `FansState` in the Snapshot drives a **Fans** section in the Metrics View. Read-only: the app
  never clears the firmware's read lock (user decision), so on the developer's ASUS board, where
  spike S1 found the lock set, the section shows `— locked by firmware`.

The CPU Fan row of the Desktop Widget (US3) depends on known-board labels, a table that starts
empty because no board could be checked yet (research R6).

## Technical Context

**Language/Version**: Rust stable, edition 2024

**Primary Dependencies**: `slint` 1.18 and `windows` 0.62, features unchanged (`CreateMutexW` and
`WaitForSingleObject` are in `Win32_System_Threading`, already enabled). No new crates.

**Storage**: none (research R6 reads the board name from `HKLM\HARDWARE\DESCRIPTION\System\BIOS`)

**Testing**: `cargo test` for `domain::fan` and the new metric values; manual scenarios F1–F10
([quickstart.md](quickstart.md)); `scripts/measure.ps1`

**Target Platform**: Windows 10 1809+ / Windows 11 x64, elevated

**Project Type**: Desktop tray app (single binary)

**Performance Goals**: fan read < 1 ms per tick (research R8); detection < 50 ms once

**Constraints**: background ≤ 30 MB private working set and < 0.5 % CPU; exe ≤ 10 MB (8.99 MiB in
0.3.0); the sampler never waits more than 10 ms for the ISA mutex

**Scale/Scope**: ~450 lines of Rust, ~40 lines of `.slint`, one more shipped module file

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How |
|-----------|--------|-----|
| I. Resource Budgets | ✅ | < 1 ms of port I/O per second; at most 8 extra `History` buffers; measured in F10 before release |
| II. Native Only | ✅ | PawnIO and D3DKMT, already used; no web view, no helper process |
| III. Unsafe Isolation | ✅ | Port I/O, mutex and registry read live in `src/platform/superio.rs` and `pawnio.rs`; `domain::fan` is pure and keeps `#![deny(unsafe_code)]` |
| IV. Elevated Minimal Surface | ✅ | No network, no processes. `LpcIO.bin` is a pinned, shipped PawnIO module like the CPU Temperature ones, not a runtime plugin. Chip writes are limited to the access protocol (spec FR-006, research R5) |
| V. Software Rendering | ✅ | No rendering change |
| VI. Dependency Discipline | ✅ | No new crates or `windows` features |
| VII. Sampling Discipline | ✅ | Reads run in the existing sampler tick; fixed-size `History` per fan; at most 8 fans |
| VIII. Graceful Degradation | ✅ | Fans and GPU Fan fail independently of each other and of every Metric; every failure maps to a `Reason`; no panics in the sampling path |
| IX. Code Hygiene | ✅ | "Fan Speed", "GPU Fan", "CPU Fan" and "Fans" go into `CONTEXT.md` before code |

**Constitution text**: the *Technical Constraints → Data sources* list names PawnIO for CPU
Temperature only. A PATCH amendment (1.2.0 → 1.2.1) adds "PawnIO `LpcIO` module (motherboard Fan
Speeds)" and "D3DKMT perf data (GPU Fan)". It is a clarification, not a new capability, so no ADR.

**Post-design re-check**: passing.

## Project Structure

### Documentation (this feature)

```text
specs/006-fan-speeds/
├── plan.md              # This file
├── research.md          # R1–R8, spikes S1 (Super I/O) and S2 (GPU fan)
├── data-model.md        # domain::fan, Snapshot changes, FanSource lifecycle
├── quickstart.md        # F1–F10
├── contracts/ui.md      # Panel and Desktop Widget rows
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
src/
├── domain/
│   ├── fan.rs            # NEW: chip table, count → RPM, FanList, labels, GPU Fan rule
│   ├── metric.rs         # MetricKind::GpuFan; Reason::{UnsupportedMotherboard, LockedByFirmware}
│   ├── format.rs         # rpm()
│   └── sparkline.rs      # scale(GpuFan) and the fan scale 0–3000
├── platform/
│   ├── pawnio.rs         # PawnIo::call with slices; open() shared
│   ├── superio.rs        # NEW: FanSource (detect, lock re-check, read), ISA mutex, board name
│   └── gpu.rs            # FanRPM into GpuSample
├── sampler.rs            # FanSource on the sampler thread; FansState per tick
├── snapshot.rs           # metrics: [MetricState; 7]; fans: FansState
├── ui.rs                 # Fans rows to the Panel
└── desktop_widget.rs     # GPU Fan (and CPU Fan) rows
ui/
├── metrics-view.slint    # GPU Fan row; Fans section
└── widget.slint          # Fan rows in the CPU and GPU groups
scripts/package.ps1       # LpcIO.bin pinned (research R7)
installer/computer-stats.iss  # ship LpcIO.bin
examples/fan_spike.rs     # spike, kept for diagnosing new boards
CONTEXT.md, .specify/memory/constitution.md, README.md
```

**Structure Decision**: the existing single crate. The new platform file mirrors `pawnio.rs`'s
CPU Temperature source; the new domain file holds every rule that can be tested without hardware.

## Complexity Tracking

None.
