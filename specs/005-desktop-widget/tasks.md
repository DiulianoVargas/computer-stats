---

description: "Task list for the Desktop Widget feature"
---

# Tasks: Desktop Widget

**Input**: Design documents from `/specs/005-desktop-widget/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ui.md, quickstart.md

**Tests**: As in features 002–003, every pure module in `src/domain/` gets inline `#[cfg(test)]`
unit tests in the task that creates it. Platform and UI code is validated with quickstart.md
(W1–W16).

**Organization**: Grouped by user story: US1 show Metrics on the desktop, US2 place it and keep
it there, US3 fit any screen and theme.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1–US3
- Paths are relative to the repository root

## Conventions

- Same as `specs/003-processes-view/tasks.md`: use the `CONTEXT.md` terms (Desktop Widget, Metric,
  Sparkline, Warning Level, Tray Menu, Panel). `unsafe` lives only in `src/platform/`. No
  `unwrap`/`expect` outside tests. Every failure degrades to a value: the Desktop Widget stays off
  and the tray and Panel are unaffected (FR-013).
- The Panel, Tray Icon, Status Dot and tooltip code paths are not changed except where a task
  says so (FR-012).
- The Desktop Widget is off by default and nothing related to it is created while it is off.

---

## Phase 1: Setup

- [ ] T001 Add `windows` feature `Win32_UI_Accessibility` to `Cargo.toml` with the one-line justification "`SetWinEventHook` for the Desktop Widget on Show desktop (005 research R4)"; confirm `cargo build` and `cargo deny check` pass
- [ ] T002 [P] Create `ui/widget.slint` with a placeholder `export component WidgetWindow inherits Window` (`background: #00000001`, `no-frame: true`, five `in property <MetricRowData>`: `cpu-usage`, `cpu-temperature`, `gpu-usage`, `gpu-temperature`, `memory-usage`; `in property <bool> dark`; one plain Text per Metric). Import `MetricRowData` from `metric-row.slint`, and add `export { WidgetWindow } from "widget.slint";` to `ui/panel.slint`. Confirm `cargo build` generates both `PanelWindow` and `WidgetWindow` in `crate::generated` (research R8)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The rendering pipeline. A never-shown Slint component is rendered and presented on our
own layered window. Spike S1 proves it before any behaviour is built on top.

- [ ] T003 [P] Add `pub fn apps_use_light_theme() -> bool` to `src/platform/theme.rs`. It reads `AppsUseLightTheme` from the same `Personalize` key as `taskbar_uses_light_theme`. A missing value means light (`true`), which is the Windows default for apps (research R7)
- [ ] T004 [P] Add `pub fn primary_work_area() -> Option<Rect>` to `src/platform/shell.rs`: the work area of the monitor at (0, 0) (`MonitorFromPoint(MONITOR_DEFAULTTOPRIMARY)` + `GetMonitorInfoW`), using the existing `from_win`
- [ ] T005 [P] Create `src/domain/widget_place.rs` (register it in `src/domain/mod.rs`). It holds `pub struct WidgetSettings { pub shown: bool, pub position: Option<Point> }` (`Default` = `shown: false, position: None`) and `pub fn default_position(work_area: Rect, size: Size, inset: i32) -> Point`, which returns the top-right corner inset by `inset` on the top and right edges. Unit tests: normal work area; a work area not at the origin (second monitor on the left, negative x); inset 0
- [ ] T006 Create `src/platform/desktop_window.rs` (register it in `src/platform/mod.rs`) with `pub struct DesktopWindow`:
  - **`create(on_event: impl Fn(WidgetEvent) + 'static) -> Result<DesktopWindow, String>`**: registers a window class once and creates a `WS_POPUP` window with `WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE` and **no owner** (research R3).
  - **`present(&mut self, rgba: &[u8], width: u32, height: u32, top_left: Point)`**: converts straight RGBA to premultiplied BGRA into a reused 32-bpp top-down DIB section, re-created only when the size changes, then calls `UpdateLayeredWindow(ULW_ALPHA, AC_SRC_ALPHA, SourceConstantAlpha 255)` at `top_left`.
  - **`show()`** (`ShowWindow(SW_SHOWNOACTIVATE)` then `SetWindowPos(HWND_BOTTOM, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE)`), **`hide()`**, and **`rect() -> Option<Rect>`**.
  - **`Drop`**: `DestroyWindow` and frees the DIB.
  - **`pub enum WidgetEvent`**: `Moved(Point)`, `DpiChanged(f32)`, `DisplayChanged`, `DesktopForeground(bool)`. They are only declared here and wired in later tasks.
  - Same safety-comment style as `src/platform/tray.rs`. Every failure is returned as `Err`/ignored; nothing panics.
- [ ] T007 Create `src/desktop_widget.rs` (`#![deny(unsafe_code)]` inherited; register `mod desktop_widget;` in `src/lib.rs`) with `pub struct DesktopWidget { component: WidgetWindow, window: DesktopWindow, scale: f32, top_left: Point }` and these functions:
  - **`new(top_left, dark) -> Result<Self, String>`**: creates both.
  - **`render(&mut self, snapshot: &Snapshot)`**:
    1. Sets the five properties with the Panel's row builder. Make `metric_row` in `src/ui.rs` `pub(crate)` so both use it (SC-005).
    2. Dispatches `slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: self.scale }` when the scale changed.
    3. Sets the window size to the preferred logical size × scale.
    4. Calls `take_snapshot()` and passes the result to `DesktopWindow::present`.

    A failed snapshot skips that frame.
  - **`show()` / `hide()`**.

  The scale comes from `shell::scale_for` of the widget rect.
- [ ] T008 Spike S1 (research R1). In debug builds, `COMPUTER_STATS_TEST_WIDGET=1` turns the Desktop Widget on at start, at `default_position(primary_work_area, …)`; this is wired temporarily in `src/ui.rs` and replaced by T013. Check four things:
  - (a) text edges show partial alpha over a coloured wallpaper, with no black or white fringe;
  - (b) on a 150 % monitor the widget is 1.5× its logical size and sharp;
  - (c) `render` takes < 2 ms (log `Instant` timings to the crash log once, then remove the logging);
  - (d) T002's two generated components work together.

  Record the results under a new "Spike S1 result" heading in `specs/005-desktop-widget/research.md`. If a never-shown component cannot be snapshotted or scaled, switch `src/desktop_widget.rs` to the cloaked fallback (show the Slint window with `DWMWA_CLOAK` set, on the widget's monitor) and record that too

**Checkpoint**: with the debug variable set, the five Metrics render over the wallpaper and update
every second. Nothing else changes.

---

## Phase 3: User Story 1 - Show Metrics on the desktop (Priority: P1) 🎯 MVP

**Goal**: A Tray Menu item turns a transparent, live Desktop Widget on and off. It stays behind
every window, is never in the taskbar or Alt+Tab, and stays visible on Win+D.

**Independent Test**: Turn it on from the Tray Menu, run a CPU load, and confirm the widget's CPU
Usage and Sparkline rise with the Panel closed (W2–W3).

- [ ] T009 [US1] Build the real layout in `ui/widget.slint` per contracts/ui.md:
  - **`component HaloText`**: a Text drawn first as four ±1 px offset copies in the halo colour, then the main copy; same `text`, `color`, `horizontal-alignment` and `font-size` inputs.
  - **`component HaloMetricRow`**: the same columns and widths as `MetricRow` (label min 80 px, value stretch, secondary 36 px, 60×20 Sparkline). The Sparkline Path is drawn twice: a 3 px halo stroke under the 1.5 px line.
  - **Sections**: "CPU" (Usage, Temperature), "GPU" (Usage, Temperature) and "Memory" (Usage), with the Panel's section font size and gaps.
  - **Colours**, driven by `dark`: dark mode uses content `#ffffff`, muted `#c8c8c8`, halo `#000000` at 70 %; light mode uses content `#1b1b1b`, muted `#424242`, halo `#ffffff` at 70 %. Warning/Critical use the Panel's values for the mode: dark `#f5a623` / `#e5484d`, light `#b26b00` / `#c62828`.
  - **Window**: background stays `#00000001`, no card (FR-010); width 280 px, height from content (≈ 190 px).
- [ ] T010 [US1] Add the window behaviour in the `src/platform/desktop_window.rs` window procedure:
  - `WM_NCHITTEST` → `HTCAPTION` (drag anywhere, FR-015).
  - `WM_MOUSEACTIVATE` → `MA_NOACTIVATE` (FR-005).
  - `WM_WINDOWPOSCHANGING` → force `hwndInsertAfter = HWND_BOTTOM` and clear `SWP_NOZORDER`, unless a `raised` flag is set (FR-004, research R3).
  - Keep `raised` in the window's user data next to the event callback.
- [ ] T011 [US1] Add the Win+D handling in `src/platform/desktop_window.rs` (research R4). When the window is created, install `SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, None, proc, 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS)`; remove it with `UnhookWinEvent` in `Drop`. In the hook:
  - When the foreground window's class is `WorkerW` or `Progman`, set `raised` and call `SetWindowPos(HWND_TOP, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW)`.
  - Otherwise, clear `raised` and send it back to `HWND_BOTTOM`.
  - Emit `WidgetEvent::DesktopForeground(bool)` in both cases.
  - Match on class names, not stored HWNDs, so an Explorer restart keeps working.
- [ ] T012 [US1] Add the Tray Menu item in `src/ui.rs`:
  - **Menu**: `MENU_DESKTOP_WIDGET = 3`, with "Show desktop widget" as a checkable item between "Start with Windows" and the separator (contracts/ui.md).
  - **State**: add `desktop_widget: Option<DesktopWidget>` to `App`. Toggling builds or drops it. On build, place it at `default_position(primary_work_area, size, 24 logical px × scale)`, call `render(&app.latest)` before `show()` so the first frame already has values, and `dark` = `!theme::apps_use_light_theme()`.
  - **Failure**: if `DesktopWidget::new` fails, the item stays unchecked and the error goes to `crash_log::write` once.
  - **Updates**: in `on_snapshot`, render the widget after `app.latest = snapshot` whenever it is `Some`, independent of `app.visible` (FR-003).
  - **Cleanup**: remove T008's temporary debug start-up wiring.
- [ ] T013 [US1] Spike S2 (research R4). Run W6 on Windows 11 24H2+ and Windows 10 22H2, plus W15 (Explorer restart). Record under "Spike S2 result" in `specs/005-desktop-widget/research.md` whether the widget stays visible on Win+D on each build. If it does not on a build, confirm the FR-014 fallback there: the widget hides and returns with the windows, keeps its position, and logs nothing
- [ ] T014 [US1] Validate quickstart W1–W6 and W14 and note the results in `specs/005-desktop-widget/quickstart.md` (pass/fail column or notes below the table)

**Checkpoint**: the MVP. The Desktop Widget is optional, live, transparent and stays behind other
windows. The Panel behaves exactly as in 0.2.0.

---

## Phase 4: User Story 2 - Place it and keep it there (Priority: P2)

**Goal**: The dropped position is kept inside a monitor's work area. On/off and position survive
restarts and monitor changes.

**Independent Test**: Drag it to the second monitor, restart the app, and confirm it is in the
same place. Turn it off, restart, and confirm it stays off (W8–W9).

- [ ] T015 [P] [US2] Add `pub fn clamp(top_left: Point, size: Size, work_area: Rect) -> Point` to `src/domain/widget_place.rs`. It returns "the closest position that keeps the whole rectangle inside `work_area`. If `size` is larger than the work area, top-left is pinned to the work area's top-left" (data-model.md). Unit tests: inside → unchanged; off each of the four edges → pulled in; fully off-screen (old monitor) → on the work area; overlapping the taskbar (bottom of the work area) → above it; larger than the work area → pinned
- [ ] T016 [P] [US2] Create `src/platform/settings.rs` (register it in `src/platform/mod.rs`) with:
  - `pub fn load() -> WidgetSettings`: reads DWORDs `DesktopWidget` (0/1), `DesktopWidgetX` and `DesktopWidgetY` (i32 stored as two's-complement DWORD) under `HKCU\Software\Computer Stats` with `RegGetValueW`. A missing or unreadable `DesktopWidget` gives `WidgetSettings::default()`, and a missing X or Y gives `position: None` (research R5).
  - `pub fn save_shown(bool)` and `pub fn save_position(Point)` via `RegSetKeyValueW` (`REG_DWORD`). The key is created on first write, and failures are ignored.
- [ ] T017 [US2] Report moves and display changes from the `src/platform/desktop_window.rs` window procedure:
  - `WM_EXITSIZEMOVE` → `WidgetEvent::Moved(top-left from GetWindowRect)`.
  - `WM_DISPLAYCHANGE`, and `WM_SETTINGCHANGE` with `wParam == SPI_SETWORKAREA` → `WidgetEvent::DisplayChanged`.

  Dispatch through the same deferred path the tray uses, so no App borrow is held during the callback.
- [ ] T018 [US2] Add `pub fn on_event(&mut self, event: WidgetEvent)` to `src/desktop_widget.rs`. On `Moved` or `DisplayChanged`, it computes `clamp(top_left, size, shell::work_area_for(rect))`. If the result differs, it re-presents at the clamped position (FR-006, FR-008). On `Moved` it then calls `settings::save_position`. `DesktopWidget::new` takes an `Option<Point>`: a saved position is clamped first, and `None` falls back to `default_position`
- [ ] T019 [US2] Restore and save the setting in `src/ui.rs`. At start, after `APP` is set, call `settings::load()`. If `shown`, build the Desktop Widget at the saved or default position on the first event-loop turn (`slint::Timer::single_shot(Duration::ZERO, …)`). When the item is toggled, call `settings::save_shown`. Route `WidgetEvent`s to `DesktopWidget::on_event` with `update_app`
- [ ] T020 [P] [US2] Add `Root: HKCU; Subkey: "Software\Computer Stats"; Flags: uninsdeletekey` to a `[Registry]` section in `installer/computer-stats.iss`
- [ ] T021 [US2] Validate quickstart W7–W9, W11 and W15 and note the results in `specs/005-desktop-widget/quickstart.md`

**Checkpoint**: the widget is "fixed". It comes back where the user left it, on a visible monitor.

---

## Phase 5: User Story 3 - Fits any screen and theme (Priority: P3)

**Goal**: The widget stays sharp and correctly sized across monitor scaling and follows Windows
light/dark mode live.

**Independent Test**: Move it between a 100 % and a 150 % monitor, then switch Windows' mode, and
confirm it stays sharp, correctly sized and readable (W10, W12).

- [ ] T022 [US3] Handle DPI changes. In `src/platform/desktop_window.rs`, `WM_DPICHANGED` emits `WidgetEvent::DpiChanged(HIWORD(wParam) as f32 / 96.0)`, and the window does not apply the suggested rect itself. In `src/desktop_widget.rs`, `on_event(DpiChanged)` updates `scale`, re-renders at the new size with the top-left kept, then clamps (FR-009). The scale is also re-read with `shell::scale_for` after `Moved`, for drops across monitors without a `WM_DPICHANGED`
- [ ] T023 [US3] Follow the theme. In `src/ui.rs`, the existing `TrayEvent::AppearanceChanged` handler also calls `desktop_widget.set_dark(!theme::apps_use_light_theme())` when the widget is on. Add `set_dark(bool)` to `src/desktop_widget.rs`; it sets the property and re-renders immediately (US3 scenario 3)
- [ ] T024 [US3] Validate quickstart W10, W12 and W13 (white, black and busy-photo wallpapers in both modes, SC-008) and note the results in `specs/005-desktop-widget/quickstart.md`. If any value is unreadable, adjust the halo alpha or offset in `ui/widget.slint` and record the final values in contracts/ui.md

**Checkpoint**: all three user stories work.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T025 Run the gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo deny check`, `cargo test`. Confirm `unsafe` appears only under `src/platform/`, there is no `unwrap`/`expect` outside tests, and with the widget off no `DesktopWindow`, WinEvent hook or `WidgetWindow` exists
- [ ] T026 Measure with `scripts/measure.ps1` (W16): 10 minutes with the widget on and the Panel closed, then the widget off and the Panel closed. Record both in `specs/005-desktop-widget/measurements.md`. Budgets: ≤ 30 MB private working set and < 0.5 % CPU with the widget on; widget off unchanged from 0.2.0; exe ≤ 10 MB
- [ ] T027 [P] Update `README.md`: a Features bullet for the optional Desktop Widget (transparent, stays behind windows, remembers its place); a row in "How to use" for "Right-click the tray icon → Show desktop widget"; and the memory/CPU line if T026 changed it
- [ ] T028 [P] Update the **Tray Menu** entry in `CONTEXT.md` to list "Start with Windows, Show desktop widget, Quit"

---

## Dependencies & Execution Order

- **Order of phases**: Setup → Foundational (T008 Spike S1 gates everything after it) → US1 →
  US2 → US3 → Polish.
  - US2 and US3 both need US1's window (T010, T012) and touch the same files
    (`desktop_window.rs`, `desktop_widget.rs`), so they run in sequence.
- **Foundational**: T003, T004 and T005 run in parallel. T006 → T007 → T008.
- **US1**: T009 is in parallel with T010 and T011 (different files). T010 → T011 (same file).
  T012 comes after T009 and T011, then T013 → T014.
- **US2**: T015, T016 and T020 run in parallel. Then T017 → T018 → T019 → T021.
- **US3**: T022 and T023 run in parallel (different files, except that both touch
  `desktop_widget.rs`, so do them in sequence if one person). T024 comes last.
- **Polish**: T027 and T028 run in parallel after T026.

## Parallel Example: Foundational

```text
Task: "T003 Add apps_use_light_theme() to src/platform/theme.rs"
Task: "T004 Add primary_work_area() to src/platform/shell.rs"
Task: "T005 Create src/domain/widget_place.rs with default_position()"
```

## Parallel Example: User Story 2

```text
Task: "T015 Add clamp() to src/domain/widget_place.rs"
Task: "T016 Create src/platform/settings.rs"
Task: "T020 Add [Registry] uninsdeletekey to installer/computer-stats.iss"
```

## Implementation Strategy

1. **Setup + Foundational**: the rendering pipeline, proven by Spike S1. If S1 fails, the cloaked
   fallback is applied before going further.
2. **US1 (MVP)**: the optional widget from the Tray Menu, live and transparent, behind windows,
   staying on Win+D (Spike S2).
3. **US2**: placement kept inside monitors and persisted. This is what makes it "fixed".
4. **US3**: DPI and live theme.
5. **Polish**: gates, budgets, README and glossary.

## Notes

- Work on branch `005-desktop-widget`; commit at each checkpoint.
- Reset widget state between manual runs with `reg delete "HKCU\Software\Computer Stats" /f`.
- W6/T013 needs a Windows 10 22H2 machine or VM, in addition to Windows 11.
