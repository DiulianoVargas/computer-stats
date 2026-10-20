# Implementation Plan: Desktop Widget

**Branch**: `005-desktop-widget` | **Date**: 2026-10-20 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/005-desktop-widget/spec.md`

## Summary

The Desktop Widget is an optional extra, off by default, turned on from a new Tray Menu item. The
tray part stays exactly as it is: the Tray Icon, Status Dot, tooltip and the Panel anchored to
the taskbar.

**How it is shown**
- When it is on, the app creates its own Win32 layered window (`platform::desktop_window`) and a
  never-shown Slint `WidgetWindow` that holds five Metrics: CPU Usage and Temperature, GPU Usage
  and Temperature, and Memory Usage.
- On every Snapshot the existing sampler already sends, the app does three things:
  1. Fills the component with the same `MetricRowData` as the Panel.
  2. Renders it with `take_snapshot()`.
  3. Presents the alpha image with `UpdateLayeredWindow`, so text and Sparklines sit directly on
     the wallpaper, with a halo for contrast.

**How it behaves**
- **Click and drag:** `WM_NCHITTEST → HTCAPTION` gives native dragging, and `WS_EX_NOACTIVATE`
  keeps it from taking focus.
- **Stays behind:** a `WM_WINDOWPOSCHANGING` guard pins it to `HWND_BOTTOM`.
- **Win+D:** a WinEvent foreground hook raises it while Show desktop holds the desktop in
  front.
- **Position:** placement is clamped to the nearest monitor's work area. On/off and position
  persist in `HKCU\Software\Computer Stats`.

When it is off, nothing is created and nothing runs.

## Technical Context

**Language/Version**: Rust stable, edition 2024

**Primary Dependencies**: `slint` 1.18 (features unchanged); `windows` 0.62 with one new feature,
`Win32_UI_Accessibility` (`SetWinEventHook`, R4). No new crates.

**Storage**: registry values under `HKCU\Software\Computer Stats`: `DesktopWidget`,
`DesktopWidgetX`, `DesktopWidgetY` (R5)

**Testing**: `cargo test` for `domain::widget_place`; manual scenarios W1–W16
([quickstart.md](quickstart.md)); `scripts/measure.ps1`

**Target Platform**: Windows 10 1809+ / Windows 11 x64, elevated, PerMonitorV2

**Project Type**: Desktop tray app (single binary)

**Performance Goals**: widget frame < 2 ms (render + present) once per second; first frame
visible < 300 ms after the menu click

**Constraints**:
- Widget on, Panel closed: ≤ 30 MB private working set and < 0.5 % CPU. These are the
  constitution's background budgets, stricter than SC-004.
- Widget off: unchanged from 0.2.0.
- Exe ≤ 10 MB.

**Scale/Scope**: ~600 lines of Rust and ~120 lines of `.slint`

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How |
|-----------|--------|-----|
| I. Resource Budgets | ✅ | One small render per Snapshot only while shown; nothing exists while off; measured in W16 |
| II. Native Only | ✅ | Win32 layered window + Slint software renderer; no web view, no helper process |
| III. Unsafe Isolation | ✅ | Layered window, WinEvent hook and registry I/O live in `src/platform/` (`desktop_window.rs`, `settings.rs`) behind safe APIs; `desktop_widget.rs` keeps `#![deny(unsafe_code)]` |
| IV. Elevated Minimal Surface | ✅ | No network, no new processes, no code loaded. The WinEvent hook is out-of-context (nothing is injected into other processes). No reparenting into Explorer (R4). Writes only our own HKCU key |
| V. Software Rendering | ✅ | Transparency comes from `UpdateLayeredWindow`, not a GPU renderer (R1) |
| VI. Dependency Discipline | ✅ | No new crates; the `Win32_UI_Accessibility` feature gets a one-line justification in `Cargo.toml` |
| VII. Sampling Discipline | ✅ | No new timer or sampling. The widget repaints from the existing per-second Snapshot on the UI thread; sensor reads stay on the sampler thread |
| VIII. Graceful Degradation | ✅ | Unavailable Metrics show `—`. A failure to create, render or save the widget leaves it off and the tray/Panel intact (FR-013) |
| IX. Code Hygiene | ✅ | "Desktop Widget" was added to `CONTEXT.md` before any code; identifiers use `desktop_widget` / `DesktopWidget` |

**Post-design re-check**: passing. No amendment needed.

## Project Structure

### Documentation (this feature)

```text
specs/005-desktop-widget/
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
│   └── widget_place.rs   # NEW: WidgetSettings, default_position(), clamp() (pure, tested)
├── platform/
│   ├── desktop_window.rs # NEW: layered window, present(), wndproc (hit-test, bottom guard,
│   │                     #      move/DPI/display events), WinEvent foreground hook
│   ├── settings.rs       # NEW: read/write HKCU\Software\Computer Stats DWORDs
│   ├── shell.rs          # + primary_work_area()
│   └── theme.rs          # + apps_use_light_theme()
├── desktop_widget.rs     # NEW: DesktopWidget: owns WidgetWindow + DesktopWindow + settings;
│                         #      show/hide, update(&Snapshot), on_event()
└── ui.rs                 # Tray Menu item; App.desktop_widget; feeds Snapshots and
                          # AppearanceChanged; restores state at start
ui/
├── widget.slint          # NEW: WidgetWindow, HaloText, HaloMetricRow
└── panel.slint           # + export { WidgetWindow } from "widget.slint"
installer/computer-stats.iss  # delete HKCU\Software\Computer Stats on uninstall
Cargo.toml                # + Win32_UI_Accessibility
```

**Structure Decision**: same single crate, following the pattern of feature 003.
`desktop_widget.rs` sits next to `processes_ui.rs` as the UI-thread owner of its feature, and
`ui.rs` changes only to wire it in. The Panel code is not touched.

## Implementation Order and Risks

1. **Spike S1 (rendering), first**: render `WidgetWindow` with `take_snapshot()` without showing
   it, at scale 1.5 via `ScaleFactorChanged`, and present it on a layered window over the
   wallpaper. It must pass four checks:
   - partial alpha on text edges;
   - correct size at 150 %;
   - < 2 ms per frame;
   - both exported components generated from `panel.slint`.

   Fallback: a cloaked shown Slint window (R1).
2. **Spike S2 (Win+D)**: the foreground hook plus `HWND_TOPMOST` raise, on Windows 11 24H2+ and
   Windows 10 22H2. The result decides whether FR-014's fallback path is the norm on some builds.
   It is recorded in `research.md`.
3. Pure `widget_place` with tests, then settings, then the full window behaviour (R2, R3, R6),
   then the halo visuals (R7), then the Tray Menu wiring and restore at start.
4. **Risks**:
   - DWM may briefly show the widget above other windows during a drag. The `WM_WINDOWPOSCHANGING`
     guard is checked in W4.
   - An Explorer restart re-creates `WorkerW`/`Progman`. The hook keys on class names, not HWNDs,
     so it survives. Checked in W15.

## Complexity Tracking

No violations.
