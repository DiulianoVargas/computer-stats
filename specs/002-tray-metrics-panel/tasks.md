---

description: "Task list for the Tray Icon and Metrics Panel feature"
---

# Tasks: Tray Icon and Metrics Panel

**Input**: Design documents from `/specs/002-tray-metrics-panel/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ui.md, quickstart.md

**Tests**: The plan requires every pure module in `src/domain/` to be unit-tested (`cargo test` is a
constitution Quality Gate). Unit tests are written as inline `#[cfg(test)] mod tests` in the same
file, inside the task that creates the module. Platform and UI code is validated manually with
quickstart.md (Q1–Q15).

**Organization**: Tasks are grouped by user story so each story can be implemented and validated
on its own.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story the task belongs to (US1–US4)
- All paths are relative to the repository root (`C:\Users\Diuli\Documents\computer-stats`)

## Conventions for every task

- Identifiers use `CONTEXT.md` terms (`WarningLevel`, `StatusDot`, `Sparkline`, `Drive`, `Panel`,
  `TrayIcon`, `Metric`). Internal names allowed: `Snapshot`, `Sampler`, `History`, `Reason`.
- `unsafe` only in `src/platform/`; no `unwrap`/`expect` outside tests (clippy-denied).
- Every source returns `Reading::Unavailable(Reason)` instead of failing (Constitution VIII).
- Each new crate gets a one-line justification comment in `Cargo.toml` (Constitution VI).

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create the app crate at the repository root with its build, manifest and gates.

- [X] T001 Create `Cargo.toml` at the repository root: package `computer-stats`, edition 2024, `publish = false`; dependencies `slint = { version = "1.18", default-features = false, features = ["std", "compat-1-2", "backend-winit", "renderer-software", "raw-window-handle-06"] }`, `windows = { version = "0.62", default-features = false, features = [...] }` (start with the spike's feature list from `spikes/cpu-temp/Cargo.toml` and add `Win32_UI_WindowsAndMessaging`, `Win32_UI_Shell`, `Win32_Graphics_Gdi`, `Win32_Graphics_Dwm`, `Win32_UI_HiDpi`, `Win32_Storage_FileSystem`, `Win32_System_Ioctl`, `Win32_System_IO`, `Win32_UI_Controls` as needed), `raw-window-handle = "0.6"`; build-dependency `slint-build = "1.18"`; one-line justification comment per crate; `[profile.release]` with `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`; `[lints.clippy]` `unwrap_used = "deny"`, `expect_used = "deny"`
- [X] T002 [P] Create `deny.toml` at the repository root: bans `wry`, `webview2-com`, `tauri`, `reqwest`, `hyper`, `ureq` (and equivalents); advisories on; duplicate versions warn; license allow-list that includes Slint's `LicenseRef-Slint-Royalty-free-2.0` plus MIT/Apache-2.0/Unicode-3.0/BSD variants (start from `spikes/cpu-temp/deny.toml`)
- [X] T003 [P] Create `app.manifest` at the repository root: `requestedExecutionLevel level="highestAvailable" uiAccess="false"`, `dpiAwareness` `PerMonitorV2`, Common Controls v6 dependency, Windows 10/11 `supportedOS` GUID
- [X] T004 Create `build.rs`: `slint_build::compile("ui/panel.slint")`; on MSVC emit `cargo:rustc-link-arg-bins=/MANIFEST:EMBED` and `cargo:rustc-link-arg-bins=/MANIFESTINPUT:<abs path to app.manifest>`; `cargo:rerun-if-changed=app.manifest` (research R6; depends on T001, T003)
- [X] T005 [P] Create `.gitignore` at the repository root (`/target`, `*.bin`, `*.log`) and `README.md` with build/run notes (elevated terminal for `cargo run`, `AMDFamily17.bin` next to the exe) and the Slint royalty-free attribution required by Constitution VI
- [X] T006 Create `src/main.rs` skeleton: `#![deny(unsafe_code)]`, `#![windows_subsystem = "windows"]`, `slint::include_modules!()`, `mod domain; #[allow(unsafe_code)] mod platform; mod sampler; mod snapshot; mod ui; mod startup;` and a `main()` that returns after creating the Panel; create empty `src/domain/mod.rs`, `src/platform/mod.rs`, `src/sampler.rs`, `src/snapshot.rs`, `src/ui.rs`, `src/startup.rs` so `cargo build` succeeds (depends on T004)
- [X] T007 [P] Create `scripts/measure.ps1`: parameters `-Minutes` (default 10) and `-PanelOpen` (switch, only labels the run); samples `computer-stats` every second with `Get-Process` (`PrivateMemorySize64`, CPU time delta / logical processors); prints max and mean private MB and mean CPU %, and exits non-zero if over budget (closed: 30 MB / 0.5%; open: 45 MB / 2%); also prints the size of `target\release\computer-stats.exe` and fails if > 10 MB (Constitution I)

**Checkpoint**: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo deny check`, `cargo build` pass.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core types, the sampler thread, the Tray Icon and a hidden Panel — the running
skeleton every story builds on, measured before any Metric is added (plan: "skeleton + measurement first").

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T008 [P] Create `src/domain/metric.rs` (+ `pub mod metric;` in `src/domain/mod.rs`, which declares `#![deny(unsafe_code)]`-compatible pure modules only): `MetricKind` = `CpuUsage | CpuTemperature | GpuUsage | GpuTemperature | Vram | MemoryUsage` with `ALL` in Panel order (CpuUsage, CpuTemperature, GpuUsage, GpuTemperature, Vram, MemoryUsage), `display_name()` ("CPU Usage", "CPU Temperature", "GPU Usage", "GPU Temperature", "VRAM", "Memory Usage") and `is_temperature()`; `Reading` = `Value { value: f32, used_bytes: Option<u64>, total_bytes: Option<u64> }` ("bytes only for MemoryUsage and Vram") or `Unavailable(Reason)`; `Reason` = `NotElevated | UnsupportedCpu | DriverNotFound | ModuleNotFound | NoGpu | ReadFailed` with `message()` returning exactly "not elevated", "unsupported CPU", "driver not found", "module not found", "no GPU", "read failed"; constructor `Reading::validated(kind, value, ...)` applying "percentages are clamped to 0–100; temperatures outside 0–115 °C become `Unavailable(ReadFailed)`"; unit tests for clamping, the 0/115 °C boundaries and every message
- [X] T009 [P] Create `src/snapshot.rs`: `MetricState { kind: MetricKind, reading: Reading }`, `DriveState` placeholder (filled in T020), `Snapshot { metrics: [MetricState; 6], drives: Vec<DriveState>, tooltip: String }` with `Clone`; `tooltip` is `"Computer Stats"` for now. Later stories add fields (`level`, `history`, `status_dot`) without changing existing ones
- [X] T010 Create `src/sampler.rs`: `Sampler::spawn(on_snapshot: impl Fn(Snapshot) + Send + 'static) -> SamplerHandle`; a dedicated thread that sleeps until the next whole-second deadline (`Instant`-based, no busy loop, Constitution VII), builds a `Snapshot` (all Metrics `Unavailable(ReadFailed)` for now) and calls `on_snapshot`; `SamplerHandle::stop()` sets an `AtomicBool` and joins with a 2 s limit (research R10); the tick function never panics (depends on T008, T009)
- [X] T011 [P] Create `src/domain/tray_art.rs`: `TrayTheme { Light, Dark }` and `render(size_px: u32, theme: TrayTheme) -> Vec<u32>` returning premultiplied ARGB pixels of three ascending bars (white glyph for `Dark` taskbar, near-black for `Light`), transparent elsewhere; unit tests: buffer length = size², fully transparent corners, glyph pixels present at 16, 24 and 32 px (research R3)
- [X] T012 [P] Create `src/platform/theme.rs`: `taskbar_uses_light_theme() -> bool` reading `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\SystemUsesLightTheme` (missing value → dark) (research R3)
- [X] T013 Create `src/platform/tray.rs`: `TrayIcon::create(on_event: impl Fn(TrayEvent) + 'static)` that registers window class `ComputerStats.Tray`, creates a hidden top-level window on the UI thread, and adds the icon with `Shell_NotifyIconW` (`NIM_ADD` + `NIM_SETVERSION` `NOTIFYICON_VERSION_4`, `NIF_GUID` with a fixed GUID constant, `NIF_MESSAGE`, `NIF_ICON`, `NIF_TIP | NIF_SHOWTIP`); `set_icon(pixels: &[u32], size: u32)` building an `HICON` with `CreateDIBSection` + `CreateIconIndirect` (old icon destroyed); `set_tooltip(&str)` that only calls `NIM_MODIFY` when the text changed; `WndProc` maps `NIN_SELECT`/`NIN_KEYSELECT` → `TrayEvent::Select`, `WM_CONTEXTMENU` → `TrayEvent::ContextMenu { x, y }`, the registered `TaskbarCreated` message → re-add the icon (FR-024); events are forwarded with `slint::invoke_from_event_loop` (never call Slint re-entrantly); `Drop` does `NIM_DELETE`; export from `src/platform/mod.rs` (research R2; depends on T011, T012)
- [X] T014 [P] Create `ui/theme.slint`: global `Theme` with light/dark tokens switched on `Palette.color-scheme == ColorScheme.dark` (background, border, text, muted text, accent bar, bar track), spacing tokens (16 px padding, 12 px section gap) (research R12)
- [X] T015 Create `ui/panel.slint`: exported `PanelWindow` (`no-frame: true`, `always-on-top: true`, width 340 px, `max-height` 640 px, `title: "Computer Stats"`), 1 px border and rounded background from `Theme`, a root `FocusScope` that calls `callback close-requested()` on Esc, and a vertical layout with empty `CPU`, `GPU`, `Memory`, `Drives` section headers followed by an empty, commented `App Volumes` slot (FR-010) (depends on T014)
- [X] T016 Create `src/ui.rs` and wire `src/main.rs`: create `PanelWindow` (hidden), `TrayIcon` with the glyph for the current theme at `GetSystemMetricsForDpi(SM_CXSMICON)` size, start the `Sampler` whose callback stores the latest `Snapshot` in a `Rc<RefCell<Option<Snapshot>>>` via `invoke_from_event_loop`, run `slint::run_event_loop_until_quit()`; on exit stop the sampler and drop the Tray Icon (depends on T010, T013, T015)
- [X] T017 Build release, run the skeleton elevated, and record the baseline from `scripts/measure.ps1 -Minutes 2` (private MB, CPU %, exe size) in a new `specs/002-tray-metrics-panel/measurements.md`; if the skeleton alone exceeds 20 MB, stop and revisit Slint features before continuing (plan risk 1)

**Checkpoint**: The app starts, shows one Tray Icon, uses a measured baseline of memory/CPU, and can be ended from Task Manager.

---

## Phase 3: User Story 1 - Glance at all Metrics (Priority: P1) 🎯 MVP

**Goal**: Left-click opens a Panel anchored above the Tray Icon showing every Metric, updated each
second; it closes on outside click, Esc or a second click; the Tray Menu offers Quit.

**Independent Test**: Launch the app, left-click the Tray Icon, watch values update for 10 s,
click the desktop, then right-click → Quit (quickstart Q1, Q2, Q3, Q9, Q12, Q15).

### Pure modules (with unit tests)

- [X] T018 [P] [US1] Create `src/domain/format.rs`: `percent(v) -> "23 %"` (rounded, no decimals); `temperature(v) -> "54 °C"`; `used_total(used, total) -> "18.4 / 31.9 GB"` (one decimal); `size(bytes) -> "212 GB"` or `"1.4 TB"` when ≥ 1000 GB (one decimal for TB, integer GB); `drive_free(free, total) -> "212 GB free of 931 GB"`; `drive_label(letter, label) -> "C: System"` or `"C:"` when the label is empty; `unavailable(reason) -> "— <reason message>"`; "1 GB = 1024³ bytes"; unit tests for each format and rounding boundary (contracts/ui.md Rows; FR-011)
- [X] T019 [P] [US1] Create `src/domain/cpu_temp.rs` by copying `decode_tctl`, `classify` and `is_supported` from `spikes/cpu-temp/src/reading.rs` together with their tests, adapted to return `Reading`/`Reason` from `domain::metric` and to split "module not found" from "driver not found" (research R8)
- [X] T020 [P] [US1] Create `src/domain/drive.rs`: `Drive { letter: char, label: String, total_bytes: u64, free_bytes: u64 }` with `used_pct() = (total − free) / total × 100` (0 when total is 0); identity is the letter; `sort_by_letter`; and complete `DriveState` in `src/snapshot.rs` as `Drive` or `{ letter, reason: Reason }` unavailable; unit tests for `used_pct` and ordering
- [X] T021 [P] [US1] Create `src/domain/anchor.rs`: `Rect`, `Size`, `Point` (physical px), `Edge { Bottom, Top, Left, Right }`, and `place(icon: Rect, work_area: Rect, edge: Edge, size: Size) -> Point`: Panel sits 12 px from the taskbar edge, centered on the icon along the taskbar, clamped inside `work_area` with a 12 px margin; unit tests for all four edges, an icon at the far right of the work area, and a 1×1 cursor rect (research R5; FR-007)

### Metric sources (platform, unsafe)

- [X] T022 [P] [US1] Create `src/platform/cpu.rs`: `CpuTimes::new()` and `sample(&mut self) -> Reading` using `GetSystemTimes` deltas between calls, `1 − idle/(kernel+user)` × 100; first call or zero delta → `Unavailable(ReadFailed)` (research R8)
- [X] T023 [P] [US1] Create `src/platform/memory.rs`: `memory_usage() -> Reading` from `GlobalMemoryStatusEx` with `used_bytes = ullTotalPhys − ullAvailPhys`, `total_bytes = ullTotalPhys`, value = used % (research R8)
- [X] T024 [P] [US1] Create `src/platform/drives.rs`: `enumerate() -> Vec<char>` with `GetLogicalDrives` → `GetDriveTypeW == DRIVE_FIXED` → exclude volumes whose `IOCTL_STORAGE_QUERY_PROPERTY` bus type is `BusTypeUsb`; `read(letter) -> Result<Drive, Reason>` with `GetDiskFreeSpaceExW` and `GetVolumeInformationW` for the label (research R8; FR-022)
- [X] T025 [P] [US1] Create `src/platform/pawnio.rs` by copying elevation check, CPUID and the PawnIO handle from `spikes/cpu-temp/src/platform.rs`; expose `CpuTemperatureSource::open() -> Result<Self, Reason>` (opened **once** for the app's lifetime) and `read(&mut self) -> Reading` using `domain::cpu_temp`; not elevated → `NotElevated`, missing `AMDFamily17.bin` → `ModuleNotFound`, missing driver → `DriverNotFound` (depends on T019)
- [X] T026 [P] [US1] *(Replaced by `src/platform/gpu.rs` + `src/domain/gpu.rs` on D3DKMT after T028, ADR-0003.)* Create `src/platform/nvml.rs`: load `nvml.dll` with `LoadLibraryExW(LOAD_LIBRARY_SEARCH_SYSTEM32)` and resolve `nvmlInit_v2`, `nvmlDeviceGetCount_v2`, `nvmlDeviceGetHandleByIndex_v2`, `nvmlDeviceGetUtilizationRates`, `nvmlDeviceGetMemoryInfo`, `nvmlDeviceGetTemperature` (`NVML_TEMPERATURE_GPU`), `nvmlShutdown`; `GpuSource::open() -> Result<Self, Reason>` (DLL missing or count 0 → `NoGpu`, `NVML_ERROR_DRIVER_NOT_LOADED` → `DriverNotFound`), device index 0; `read(&self) -> (Reading, Reading, Reading)` for GPU Usage, GPU Temperature, VRAM, each failing independently with `ReadFailed`; `Drop` calls `nvmlShutdown` (research R9)

### Sampling, Panel and Tray Menu

- [X] T027 [US1] Update `src/sampler.rs` to own `CpuTimes`, the `CpuTemperatureSource` and `GpuSource` results (an `Err(reason)` source yields `Unavailable(reason)` every tick), read Memory Usage each tick, re-enumerate Drives every 5 s and read free space of known Drives each tick, and fill `Snapshot.metrics` (Panel order) and `Snapshot.drives` (sorted by letter); any failing source affects only its own Metric (depends on T020, T022–T026)
- [X] T028 [US1] Measure the NVML footprint: run `scripts/measure.ps1 -Minutes 2` with and without the GPU source (temporary debug flag or env var `COMPUTER_STATS_NO_NVML`) and append the delta to `specs/002-tray-metrics-panel/measurements.md`; if the closed-Panel budget is at risk, document the mitigation (lazy load / unload after N failures) in plan.md Complexity Tracking (research R9 risk; depends on T027)
- [X] T029 [P] [US1] Create `ui/metric-row.slint`: `MetricRow` with `label`, `value-text`, `secondary-text` (e.g. `18 %` for VRAM/Memory), `unavailable: bool` (value shown in muted color as `— reason`), and a reserved 60×20 px slot on the right for the Sparkline (filled in US3) (contracts/ui.md Rows)
- [X] T030 [P] [US1] Create `ui/drive-row.slint`: `DriveRow` with `label-text` (`C: System`), `used-fraction: float` drawn as a bar (accent fill on track), `free-text` (`212 GB free of 931 GB`), and unavailable state (`— reason`, muted)
- [X] T031 [US1] Update `ui/panel.slint`: import `MetricRow` and `DriveRow`; properties `cpu-usage`, `cpu-temperature`, `gpu-usage`, `gpu-temperature`, `vram`, `memory-usage` (a `MetricRowData` struct each) and `drives: [DriveRowData]`; sections CPU (Usage, Temperature), GPU (Usage, Temperature, VRAM), Memory (Usage), Drives (one row per Drive, inside a `ScrollView` whose height caps at 6 rows), App Volumes slot left empty (FR-009, FR-010; depends on T029, T030)
- [X] T032 [P] [US1] Create `src/platform/shell.rs`: `icon_rect(hwnd, guid) -> Option<Rect>` via `Shell_NotifyIconGetRect` (fallback: cursor position as 1×1 rect), `taskbar_edge() -> Edge` via `SHAppBarMessage(ABM_GETTASKBARPOS)`, `work_area_for(rect) -> Rect` via `MonitorFromRect` + `GetMonitorInfoW`, `scale_for(rect) -> f32` via `GetDpiForMonitor` / 96 (research R5)
- [X] T033 [US1] *(Done without this file: winit attributes hook + `on_winit_window_event`, see research R4 amendment.)* Create `src/platform/window.rs`: from the Panel's `raw-window-handle` HWND, after the first `show()`: add `WS_EX_TOOLWINDOW`, remove `WS_EX_APPWINDOW` (FR-001), set `DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND`; `SetWindowSubclass` that calls an `on_deactivate` callback on `WM_ACTIVATE` with `WA_INACTIVE`; `move_to(Point)` with `SetWindowPos` (physical px); `bring_to_front()` with `SetForegroundWindow` (research R4)
- [X] T034 [US1] Implement Panel open/close in `src/ui.rs`: `toggle()` — when hidden: push the latest Snapshot into the Panel properties (formatted with `domain::format`), compute size in physical px = logical size × `shell::scale_for(icon)`, position with `anchor::place`, `show()`, `bring_to_front()`; when visible: `hide()`. Deactivation hides the Panel and records `Instant`; a `TrayEvent::Select` within 300 ms of that hide is ignored (toggle race, research R4); `close-requested` (Esc) hides; Panel properties are updated on each Snapshot **only while visible** (FR-002, FR-008, FR-012, SC-001; depends on T018, T021, T031–T033)
- [X] T035 [US1] Add the Tray Menu to `src/platform/tray.rs` and handle it in `src/ui.rs`: on `TrayEvent::ContextMenu`, build a popup menu with `Quit` (Start with Windows is added in US4), `SetForegroundWindow(hidden)`, `TrackPopupMenuEx`, `PostMessageW(WM_NULL)`; Quit stops the sampler, removes the Tray Icon and quits the event loop with exit code 0 (FR-003, FR-004)
- [X] T036 [US1] Validate US1 with quickstart Q1, Q2, Q3, Q9, Q12 and Q15 and note results in `specs/002-tray-metrics-panel/measurements.md`

**Checkpoint**: MVP — the Panel shows all Metrics with live values and closes correctly; Quit works.

---

## Phase 4: User Story 2 - Know when something needs attention (Priority: P2)

**Goal**: Values colored by Warning Level, and a Status Dot on the Tray Icon showing the worst level
(Disk Space only at Critical), with a tooltip naming the cause.

**Independent Test**: Push Memory Usage past 80% → amber value and Status Dot within 2 s; release →
back to normal after dropping below 78% (quickstart Q4, Q5, Q6).

- [X] T037 [P] [US2] Create `src/domain/warning.rs`: `WarningLevel { Normal, Warning, Critical }` (`Ord`, `Normal < Warning < Critical`); `Thresholds { warning: f32, critical: f32 }` and `thresholds(MetricKind) -> Option<Thresholds>` = CpuUsage/GpuUsage `None`, MemoryUsage 80/90, Vram 85/95, CpuTemperature 80/90, GpuTemperature 80/87, plus `DISK_SPACE = 85/95` (FR-014); `next_level(prev: Option<WarningLevel>, v: f32, t: Thresholds) -> WarningLevel` implementing exactly: "1. `raw` = the highest level whose threshold `v` meets. 2. If `raw ≥ p` → new level is `raw` (rising is immediate). 3. Else, step down one level at a time while `v < threshold(current) − 2`; the result is the highest level for which that check fails." with `prev = None` treated as Normal ("An `Unavailable` reading resets the level to `None`... the next value starts from Normal"); unit tests reproduce the data-model example "Memory: 81 → Warning; 79 → Warning; 77.9 → Normal; 91 → Critical; 89 → Critical; 87.9 → Warning" and Critical → Normal in one step when far below
- [X] T038 [P] [US2] Create `src/domain/status_dot.rs`: `StatusDot { level: WarningLevel, cause: Option<String> }` and `compute(metrics: &[(MetricKind, Option<WarningLevel>)], drives: &[(char, Option<WarningLevel>)]) -> StatusDot`: "max over every Metric's level, plus each Drive's level **only if Critical**"; cause is the display name (or `Disk C:`) of the worst, "ties go to the first in Panel order"; `tooltip(&StatusDot) -> String` = `"Computer Stats"` or `"Computer Stats — <cause> <Warning|Critical>"`; unit tests for the Drive-at-Warning rule (FR-017, US2 scenario 4), Warning + Critical → Critical, tie order and tooltip text
- [X] T039 [US2] Extend `src/domain/tray_art.rs`: `render(size_px, theme, dot: WarningLevel)` draws, when not Normal, a filled lower-right dot (amber `#F5A623`, red `#E5484D`) with a 1 px ring in the taskbar background color; unit tests: no dot pixels for Normal, amber/red pixel at the dot center (research R3; depends on T037)
- [X] T040 [US2] Update `src/snapshot.rs` and `src/sampler.rs`: add `level: Option<WarningLevel>` to `MetricState` and `DriveState` and `status_dot: StatusDot` to `Snapshot`; the sampler keeps the previous level per Metric and per Drive letter, applies `next_level` each tick (`None` for unavailable readings and for CpuUsage/GpuUsage as `Normal`), computes `StatusDot` and `tooltip` (depends on T037, T038)
- [X] T041 [US2] Add Warning colors to `ui/theme.slint` (amber `#F5A623` dark / `#B26B00` light, red `#E5484D` dark / `#C62828` light) and a `level` property (`0 = none/normal, 1 = warning, 2 = critical`) to `ui/metric-row.slint` and `ui/drive-row.slint` that colors value text and Drive bar; unavailable rows ignore it (FR-016; contracts/ui.md Colors)
- [X] T042 [US2] Update `src/ui.rs`: map levels into the Panel row data; keep the last rendered `(WarningLevel, TrayTheme, size)` and the last tooltip, calling `tray.set_icon(tray_art::render(...))` and `tray.set_tooltip(...)` only when they change (FR-006, FR-017, SC-005; depends on T039–T041)
- [ ] T043 [US2] Validate with quickstart Q4, Q5 and Q6 (for Q5, a debug-only env var `COMPUTER_STATS_TEST_THRESHOLD_OFFSET` lowering temperature thresholds is acceptable if compiled only under `cfg(debug_assertions)`)

**Checkpoint**: Warning Levels visible in the Panel and on the Tray Icon; US1 still works.

---

## Phase 5: User Story 3 - See recent trend (Priority: P2)

**Goal**: A ~60 s Sparkline beside each Metric (not Disk Space), collected while the Panel is closed,
with gaps for unavailable or sleeping seconds.

**Independent Test**: Wait 60 s, run a 20 s CPU load, open the Panel — CPU Usage and CPU
Temperature Sparklines show the rise and fall (quickstart Q7).

- [X] T044 [P] [US3] Create `src/domain/history.rs`: `History`, a fixed ring buffer of 60 `Option<f32>` (`None` = gap) whose capacity never changes (Constitution VII); `push(Option<f32>)`, `push_gaps(n)` (capped at 60), `iter()` yielding oldest → newest "with only the samples pushed so far (no fabricated values before the first tick)", `len()`; unit tests for wrap-around, partial fill and gaps
- [X] T045 [P] [US3] Create `src/domain/sparkline.rs`: `path(history: &History, min: f32, max: f32) -> String` in a 60 × 20 viewbox: x = slot index aligned so the newest sample is at x = 59, y = 20 − (clamp(v) − min)/(max − min) × 20; one `M x y L …` run per contiguous block of samples so gaps break the line; a single isolated sample emits a tiny horizontal segment; empty history → empty string; `scale(kind) -> (min, max)` = 0–100 for percentages, 20–100 for temperatures; unit tests for gaps, clamping and newest-at-right (research R11)
- [X] T046 [US3] Update `src/snapshot.rs` and `src/sampler.rs`: add `history: History` to `MetricState`; the sampler owns one `History` per Metric, pushes `Some(value)` or `None` each tick, and, when more than 1.5 s of wall-clock time passed since the previous tick, pushes `min(elapsed − 1, 60)` gaps first (sleep/resume, FR-021) (depends on T044)
- [X] T047 [P] [US3] Create `ui/sparkline.slint`: `Sparkline` component, 60 × 20 px, a `Path` with `viewbox-width: 60`, `viewbox-height: 20`, `commands` bound to an input string, `stroke` bound to an input color, `stroke-width: 1.5px`, no fill
- [X] T048 [US3] Place `Sparkline` in the reserved slot of `ui/metric-row.slint` with a `sparkline-commands` property; stroke uses the same color as the value text (including Warning colors) (contracts/ui.md Colors; depends on T047)
- [X] T049 [US3] Update `src/ui.rs` to fill `sparkline-commands` for each Metric from `sparkline::path(&state.history, scale(kind))` when pushing a Snapshot to the visible Panel (depends on T045, T046, T048)
- [ ] T050 [US3] Validate with quickstart Q7, plus a sleep/resume check (Sparkline shows a gap)

**Checkpoint**: Sparklines show real history including gaps; US1 and US2 unchanged.

---

## Phase 6: User Story 4 - Start with Windows (Priority: P3)

**Goal**: A checkable Tray Menu item that creates/removes a scheduled task starting the app
elevated at sign-in without UAC.

**Independent Test**: Check the option, sign out/in → Tray Icon appears and CPU Temperature reads;
uncheck → no longer starts (quickstart Q8).

- [ ] T051 [P] [US4] Create `src/domain/task_xml.rs`: `task_xml(exe_path: &str, user_id: &str) -> String` producing Task Scheduler XML (UTF-16 declaration, written by the caller as UTF-16 LE with BOM) with: `LogonTrigger` for `user_id`, `RunLevel` `HighestAvailable`, `DisallowStartIfOnBatteries=false`, `StopIfGoingOnBatteries=false`, `ExecutionTimeLimit=PT0S`, `MultipleInstancesPolicy=IgnoreNew`, `Exec/Command` = the exe path (XML-escaped), no arguments; unit tests check each element and escaping of `&`, `<` and quotes in the path (research R7; contracts/ui.md Start with Windows task)
- [ ] T052 [US4] Implement `src/startup.rs` (safe `std::process` only, Constitution IV): `is_enabled() -> bool` via `schtasks /Query /TN "Computer Stats"` exit code; `enable() -> Result<(), String>` writes the XML to `%TEMP%\computer-stats-task.xml`, runs `schtasks /Create /TN "Computer Stats" /XML <file> /F`, deletes the file; `disable()` runs `schtasks /Delete /TN "Computer Stats" /F`; every spawn uses `CREATE_NO_WINDOW` via `std::os::windows::process::CommandExt::creation_flags(0x0800_0000)`; user id from `USERDOMAIN\USERNAME` env vars (depends on T051)
- [ ] T053 [US4] Add a checkable `Start with Windows` item, a separator and `Quit` to the Tray Menu in `src/platform/tray.rs` (exactly two items, FR-003); in `src/ui.rs` read `startup::is_enabled()` once at startup and cache it; on click call `enable()`/`disable()` and update the cache only on success, so a failure keeps the old checkmark with no dialog (FR-005; depends on T052)
- [ ] T054 [US4] Validate with quickstart Q8 (`schtasks /Query /TN "Computer Stats" /XML` matches T051's settings)

**Checkpoint**: All four user stories work independently.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Single instance, theme/DPI changes, budgets and final validation.

- [ ] T055 [P] Create `src/platform/instance.rs`: named mutex `Local\ComputerStats.SingleInstance`; if it already exists, find the window of class `ComputerStats.Tray`, post the registered message `ComputerStats.OpenPanel` and return `AlreadyRunning`; the running instance calls `ChangeWindowMessageFilterEx(MSGFLT_ALLOW)` for that message and `tray.rs` maps it to `TrayEvent::OpenPanel`; `src/main.rs` checks it before creating any window and exits with code 0, and `src/ui.rs` opens (not toggles) the Panel on `OpenPanel` (FR-023, research R6)
- [ ] T056 Handle theme and DPI changes in `src/platform/tray.rs` + `src/ui.rs`: on `WM_SETTINGCHANGE` with `"ImmersiveColorSet"` re-read `taskbar_uses_light_theme()` and on `WM_DPICHANGED`/`WM_DISPLAYCHANGE` re-read the icon size; re-render the Tray Icon (FR-025, research R3)
- [ ] T057 Run quality gates and fix findings: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo deny check`, `cargo test`; grep that `unsafe` appears only under `src/platform/` and that no `unwrap(`/`expect(` exist outside `#[cfg(test)]`
- [ ] T058 Run `scripts/measure.ps1 -Minutes 10` with the Panel closed and `-PanelOpen` with it open, and start a 24 h run for SC-003; record results and exe size in `specs/002-tray-metrics-panel/measurements.md` (Constitution I; SC-002, SC-003; quickstart Q14)
- [ ] T059 Validate the remaining scenarios quickstart Q10 (second launch), Q11 (Explorer restart) and Q13 (light/dark theme), then re-run Q1–Q9, Q12 and Q15 as a regression pass; record outcomes in `specs/002-tray-metrics-panel/measurements.md`
- [ ] T060 [P] Update `README.md` (features, prerequisites, how to enable Start with Windows) and confirm no new domain term was introduced in code without being added to `CONTEXT.md` (Constitution IX)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies.
- **Foundational (Phase 2)**: depends on Setup; blocks every story.
- **US1 (Phase 3)**: depends on Foundational. It is the MVP and provides the Panel, sources and
  sampler integration the other stories decorate.
- **US2 (Phase 4)** and **US3 (Phase 5)**: depend on Foundational and on US1's sampler (T027) and
  Panel rows (T029–T031). They touch different fields and modules, so they can run in parallel with
  each other, except shared edits to `src/sampler.rs`, `src/snapshot.rs`, `ui/metric-row.slint` and
  `src/ui.rs` (do those sequentially).
- **US4 (Phase 6)**: depends only on Foundational + the Tray Menu (T035); independent of US2/US3.
- **Polish (Phase 7)**: after the desired stories.

### Within each story

- Pure `src/domain/` modules (with their unit tests) first → platform sources → sampler → Slint
  components → `src/ui.rs` wiring → quickstart validation.

### Parallel Opportunities

- Setup: T002, T003, T005, T007 in parallel after T001.
- Foundational: T008, T009, T011, T012, T014 in parallel.
- US1: T018–T021 (pure) and T022–T026 (sources) all in parallel; T029, T030, T032 in parallel.
- US2: T037 and T038 in parallel.
- US3: T044, T045, T047 in parallel.
- US4: T051 can start any time after Setup.

---

## Parallel Example: User Story 1

```text
# Pure modules (each with inline unit tests):
Task: "T018 Create src/domain/format.rs"
Task: "T019 Create src/domain/cpu_temp.rs"
Task: "T020 Create src/domain/drive.rs"
Task: "T021 Create src/domain/anchor.rs"

# Metric sources:
Task: "T022 Create src/platform/cpu.rs"
Task: "T023 Create src/platform/memory.rs"
Task: "T024 Create src/platform/drives.rs"
Task: "T025 Create src/platform/pawnio.rs"
Task: "T026 Create src/platform/gpu.rs (D3DKMT, ADR-0003)"

# Slint components and shell helpers:
Task: "T029 Create ui/metric-row.slint"
Task: "T030 Create ui/drive-row.slint"
Task: "T032 Create src/platform/shell.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 only)

1. Phase 1 Setup → Phase 2 Foundational (measure the skeleton, T017).
2. Phase 3 US1; measure the GPU source early (T028; led to ADR-0003).
3. **Stop and validate** with quickstart Q1–Q3, Q9, Q12, Q15.

### Incremental Delivery

1. Setup + Foundational → measured skeleton.
2. US1 → usable tray monitor (MVP).
3. US2 → Warning colors and Status Dot.
4. US3 → Sparklines.
5. US4 → Start with Windows.
6. Polish → single instance, theme/DPI, budgets, full quickstart.

---

## Notes

- [P] tasks touch different files and do not depend on incomplete tasks.
- Work happens on branch `002-tray-metrics-panel`; commit after each task or phase checkpoint.
- Stop at any checkpoint to validate a story on its own.
