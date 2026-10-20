# Implementation Plan: Processes View

**Branch**: `003-processes-view` | **Date**: 2026-10-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/003-processes-view/spec.md`

## Summary

Add a second View to the Panel. While it is visible, the existing sampler thread reads every
Process in one `NtQuerySystemInformation` call per second, computes CPU % per Process from
CPU-time deltas, groups Processes into Apps by program file, resolves names and icons once per
program (cached), and adds the result to the Snapshot. The UI orders the Apps (sort column, frozen
while hovered) into a virtualized `ListView` updated in place. End uses a pure two-click
confirmation state machine and ends each Process only after re-checking its creation time, so a
reused PID can never be hit. Nothing runs while the View is hidden.

## Technical Context

**Language/Version**: Rust stable (1.99), edition 2024

**Primary Dependencies**: `slint` 1.18 (unchanged features), `windows` 0.62 with new features
`Wdk_System_SystemInformation` (process enumeration), `Win32_System_ProcessStatus` if needed; no
new crates

**Storage**: None (View and sort remembered in memory only)

**Testing**: `cargo test` for pure modules; manual scenarios P1–P14 (quickstart.md);
`scripts/measure.ps1` for budgets

**Target Platform**: Windows 11 x64, elevated

**Project Type**: Desktop tray app (single binary)

**Performance Goals**: list within 300 ms of switching (SC-001); one sample per second

**Constraints**: Panel open ≤ 45 MB / < 2% CPU with 200+ Processes; Panel closed unchanged (3.6 MB,
0.04%); exe ≤ 10 MB (currently 8.28 MiB)

**Scale/Scope**: 200–400 Processes, 100–200 Apps; ~900 lines of Rust + ~200 lines of `.slint`

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How |
|-----------|--------|-----|
| I. Resource Budgets | ✅ | One syscall per tick; per-Process handles only for new Processes; caches bounded to running programs and freed when hidden; virtualized list; measured in Polish (P13) |
| II. Native Only | ✅ | No web view, no helper process |
| III. Unsafe Isolation | ✅ | Enumeration, version info, icon extraction and termination in `src/platform/processes.rs` / `icons.rs` |
| IV. Elevated Minimal Surface | ✅ after amendment | Ending Processes is a new capability of an elevated process: bounded by the new rule "Apps MUST be ended only in response to direct, confirmed user input in the Panel" (R11, constitution 1.2.0). No network, no new spawned processes |
| V. Software Rendering | ✅ | Unchanged |
| VI. Dependency Discipline | ✅ | No new crates; two `windows` features justified in `Cargo.toml` |
| VII. Sampling Discipline | ✅ | Timer-driven sampler thread; reads off the UI thread; fixed tick; caches pruned each tick |
| VIII. Graceful Degradation | ✅ | Unreadable path/name/icon/memory falls back per Process; End reports `access denied` per row; enumeration failure shows an empty list with a reason |
| IX. Code Hygiene | ✅ | New terms (Process, Processes View, Metrics View, End App) added to `CONTEXT.md` before code |

**Post-design re-check**: passing; the constitution was amended to 1.2.0 together with this
plan (Principle IV, End App rule).

## Project Structure

### Documentation (this feature)

```text
specs/003-processes-view/
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
src/
├── domain/
│   ├── apps.rs           # NEW: ProcessId, ProcessSample, AppKey, AppUsage; grouping + CPU %
│   ├── app_order.rs      # NEW: Sort, order() with hover freeze
│   ├── end_confirm.rs    # NEW: EndConfirm state machine
│   └── format.rs         # + memory(bytes) "812 MB" / "1.4 GB"
├── platform/
│   ├── processes.rs      # NEW: NtQuerySystemInformation walk, image path, end(ProcessId…)
│   └── icons.rs          # NEW: FileDescription, small icon → RGBA
├── sampler.rs            # Command channel (Stop | ProcessesVisible), ProcessesSample per tick
├── snapshot.rs           # + processes: Option<ProcessesSample>
└── ui.rs                 # View + sort state, ListView model in place, icons cache, End flow
ui/
├── panel.slint           # tabs; Metrics View extracted; Processes View
├── metrics-view.slint    # NEW: the 002 sections, moved unchanged
├── processes-view.slint  # NEW: header + ListView of AppRow
└── app-row.slint         # NEW: icon, name (n), CPU, Memory, End/Confirm? button
```

**Structure Decision**: Same single crate. The Metrics View moves into its own `.slint` file
unchanged so the Panel becomes a tab bar plus two Views.

## Implementation Order and Risks

1. **Enumeration first**: measure one `NtQuerySystemInformation` tick
   (time and memory) with 300+ Processes before building UI.
2. **Icon extraction cost** (R5) is the main SC-001 risk; the per-tick time budget mitigates it.
3. **ListView in-place updates** must keep scroll position (P14); verified early with a fake model.
4. **PID reuse** is handled by the create-time check (R7); covered by a unit test on the pure part
   and manual P8.

## Complexity Tracking

No violations (the IV amendment is the required governance step, not a deviation).
