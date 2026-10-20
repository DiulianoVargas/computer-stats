# Research: Tray Icon and Metrics Panel

## R1. Slint configuration

- **Decision**: `slint = "1.18"` with `default-features = false` and features `std`,
  `compat-1-2`, `backend-winit`, `renderer-software`, `raw-window-handle-06`. UI is written in
  `.slint` files compiled by `slint-build` in `build.rs`.
- **Rationale**: Constitution V (software renderer) and VI (minimal features). The winit backend
  presents the software framebuffer through `softbuffer`, so no GPU context is created and the app
  does not show up in its own GPU Usage/VRAM readings. `raw-window-handle-06` exposes the Panel's
  HWND, which `platform` needs (R4).
- **Alternatives considered**: `renderer-femtovg` / `renderer-skia` (GPU, more RAM, shows in VRAM);
  `backend-qt` (huge runtime).

## R2. Tray Icon without a tray crate

- **Decision**: Implement the Tray Icon directly on Win32 in `platform::tray`: a hidden
  message-only-style window (regular hidden window, so it receives broadcast `TaskbarCreated`)
  created on the main thread, `Shell_NotifyIconW` with `NOTIFYICON_VERSION_4`, and a stable GUID
  so Windows keeps the user's "show in tray" choice.
  - Left click: `NIN_SELECT` / `NIN_KEYSELECT` → toggle Panel.
  - Right click: `WM_CONTEXTMENU` → Tray Menu with `TrackPopupMenuEx` (preceded by
    `SetForegroundWindow(hidden)` and followed by `PostMessageW(WM_NULL)` so the menu dismisses
    correctly).
  - Tooltip: `NIF_TIP | NIF_SHOWTIP`, updated only when its text changes.
  - Explorer restart (FR-024): `RegisterWindowMessageW("TaskbarCreated")` → re-add the icon.
- **Why it works with Slint**: winit runs a standard `GetMessage/DispatchMessage` loop on the main
  thread, so a window created on that thread has its `WndProc` called by Slint's loop. The
  `WndProc` never calls into Slint directly; it queues work with
  `slint::invoke_from_event_loop` to avoid re-entrancy.
- **Rationale**: Constitution VI. The `tray-icon` crate pulls `muda` and a menu abstraction for a
  two-item menu we can build in ~150 lines with the `windows` crate we already have.
- **Alternatives considered**: `tray-icon` + `muda` (more dependencies, its own event channel and
  thread model to bridge); `trayicon` crate (unmaintained).

## R3. Tray Icon art and the Status Dot

- **Decision**: Draw the icon in code. A pure function
  `tray_art::render(size_px, TrayTheme, StatusDot) -> Vec<u32>` returns premultiplied ARGB pixels
  of a simple glyph (three ascending bars) plus, when not Normal, a filled dot in the lower right
  (amber `#F5A623`, red `#E5484D`) with a 1 px ring in the taskbar background color.
  `platform::tray` turns the pixels into an `HICON` with `CreateDIBSection` + `CreateIconIndirect`.
- Icons are rendered at `GetSystemMetricsForDpi(SM_CXSMICON, dpi)` and re-rendered on
  `WM_DPICHANGED` / `WM_SETTINGCHANGE`. Only the three Status Dot variants for the current theme are
  cached (≈3 × 32×32×4 B).
- **Theme**: the taskbar theme comes from `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\SystemUsesLightTheme`
  (re-read on `WM_SETTINGCHANGE` with `"ImmersiveColorSet"`); glyph is white on dark taskbars and
  near-black on light ones.
- **Rationale**: No `.ico` assets or image crates; crisp at any DPI; the pixel function is
  unit-testable.

## R4. The Panel window

- **Decision**: One Slint `Window` (`PanelWindow`), created at startup, kept hidden, and shown on
  demand. Properties: `no-frame: true`, `always-on-top: true`, fixed width 340 logical px, height
  from content (max 640 logical px; the Drives section scrolls beyond 6 Drives).
- After the first `show()`, `platform::window` uses the HWND (from `raw-window-handle`) to:
  - add `WS_EX_TOOLWINDOW` and remove `WS_EX_APPWINDOW` (no taskbar button, not in Alt+Tab; FR-001);
  - apply rounded corners with `DwmSetWindowAttribute(DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND)`;
  - subclass the window (`SetWindowSubclass`) to catch `WM_ACTIVATE` with `WA_INACTIVE` → hide
    (click outside, FR-008).
- **Amended during implementation (2026-10-20)**: the HWND tweaks above are done through Slint's
  `unstable-winit-030` feature instead of Win32 calls: the winit attributes hook sets the hidden
  tray window as the Panel's **owner** (owned windows get no taskbar button and are not in Alt+Tab),
  `with_skip_taskbar(true)` and `with_corner_preference(Round)`; `on_winit_window_event` delivers
  `Focused(false)` for click-outside. No subclassing, no `platform/window.rs`, no
  `raw-window-handle` crate. The Panel is re-positioned when its height changes, because the Drive
  rows only count toward the height once the window exists.
- **Opening**: on `NIN_SELECT`, push the latest Snapshot into the Panel's properties, position it,
  `show()`, then `SetForegroundWindow` (allowed: the tray click grants foreground rights).
- **Toggle race**: clicking the Tray Icon while the Panel is open first deactivates the Panel
  (hiding it), then delivers `NIN_SELECT`. A hide caused by deactivation records its time; a
  `NIN_SELECT` within 300 ms of it is ignored, so the click closes instead of reopening.
- **Esc**: handled in Slint with a `FocusScope` on the root.
- **Rationale**: Keeping the window alive makes opening instant (SC-001). Hiding costs nothing
  since rendering stops while hidden.

## R5. Panel positioning

- **Decision**: A pure function `anchor::place(icon: Rect, work_area: Rect, edge: Edge, size: Size) -> Point`
  (physical pixels):
  - `edge` (taskbar side) from `SHAppBarMessage(ABM_GETTASKBARPOS)`;
  - `icon` from `Shell_NotifyIconGetRect`; if that fails (icon in the overflow flyout), the cursor
    position is used as a 1×1 rect;
  - `work_area` from `MonitorFromRect(icon)` + `GetMonitorInfoW`;
  - The Panel sits 12 px from the taskbar edge, centered on the icon along the taskbar, and is
    clamped inside `work_area` with a 12 px margin.
- Panel size in physical px = logical size × the target monitor's scale
  (`GetDpiForMonitor` / 96), so positioning is right before Slint has moved the window to that
  monitor.
- **Rationale**: Covers bottom/top/left/right taskbars and multi-monitor (spec edge cases) with a
  testable function.

## R6. Process manifest, elevation and single instance

- **Decision**: Embed an application manifest via MSVC linker args in `build.rs`
  (`/MANIFEST:EMBED`, `/MANIFESTINPUT:app.manifest`), no extra crate. The manifest declares
  `requestedExecutionLevel level="highestAvailable"`, PerMonitorV2 DPI awareness, Common Controls
  v6 and `windowsSubsystem` via `#![windows_subsystem = "windows"]` (no console).
- `highestAvailable`: an admin user gets one UAC prompt and runs elevated (ADR-0002); a standard
  user still runs, with CPU Temperature showing `— not elevated` (spec edge case).
- **Single instance (FR-023)**: named mutex `Local\ComputerStats.SingleInstance`. If it already
  exists, the new process finds the running instance's hidden window by its class name
  (`ComputerStats.Tray`), posts the registered message `ComputerStats.OpenPanel`, and exits with
  code 0. The running instance allows that message through UIPI with
  `ChangeWindowMessageFilterEx(MSGFLT_ALLOW)`.
- **Development note**: with this manifest, `cargo run` must be started from an elevated terminal
  (as in spike 001), otherwise Windows returns `ERROR_ELEVATION_REQUIRED`.

## R7. Start with Windows

- **Decision**: A Task Scheduler task named `Computer Stats`, managed by running `schtasks.exe`
  (the one process Constitution IV allows) with `CREATE_NO_WINDOW`:
  - Enable: write task XML to `%TEMP%\computer-stats-task.xml`, run
    `schtasks /Create /TN "Computer Stats" /XML <file> /F`, delete the file.
  - Disable: `schtasks /Delete /TN "Computer Stats" /F`.
  - State: `schtasks /Query /TN "Computer Stats"` exit code, checked once at startup and cached;
    the cache is updated after each successful change, so opening the Tray Menu never spawns a
    process.
- The XML (built by a pure, unit-tested function) uses a `LogonTrigger` for the current user,
  `RunLevel=HighestAvailable`, `DisallowStartIfOnBatteries=false`,
  `StopIfGoingOnBatteries=false`, `ExecutionTimeLimit=PT0S`, `MultipleInstancesPolicy=IgnoreNew`,
  and the current executable path as the action.
- **Rationale**: A scheduled task is the standard way to start elevated at sign-in without UAC
  (ADR-0002). XML is needed because `schtasks /Create /SC ONLOGON` defaults to "don't start on
  battery" and a 72-hour time limit, both wrong for a tray app.
- **Alternatives considered**: Task Scheduler COM API (`ITaskService`; no process spawn, but much
  more unsafe COM code for the same result); `HKCU\...\Run` (cannot start elevated).

## R8. Metric sources

| Metric | Source | Unavailable reasons |
|--------|--------|---------------------|
| CPU Usage | `GetSystemTimes` deltas between ticks: `1 − idle/(kernel+user)` | read failed |
| Memory Usage | `GlobalMemoryStatusEx` (`ullTotalPhys`, `ullAvailPhys`) | read failed |
| Disk Space | `GetLogicalDrives` → `GetDriveTypeW == DRIVE_FIXED` → exclude USB bus (`IOCTL_STORAGE_QUERY_PROPERTY`, `BusTypeUsb`) → `GetDiskFreeSpaceExW`, `GetVolumeInformationW` (label) | read failed (per Drive) |
| CPU Temperature | PawnIO + `AMDFamily17.bin`, from spike 001 (`platform.rs`, `reading.rs`) | not elevated, unsupported CPU, driver not found, module not found, read failed |
| GPU Usage, VRAM, GPU Temperature | NVML (R9) | no GPU, driver not found, read failed |

- Drive enumeration runs every 5 s (FR-022); free space of known Drives is read every tick.
- PawnIO is opened **once** for the app's lifetime and shared (spike verdict: ~7 MB).
- "module not found" is split from "driver not found", as the spike verdict recommended.

## R9. NVML

- **Decision**: Load `nvml.dll` at runtime (`LoadLibraryExW` with `LOAD_LIBRARY_SEARCH_SYSTEM32`;
  modern NVIDIA drivers install it in System32) and resolve `nvmlInit_v2`,
  `nvmlDeviceGetCount_v2`, `nvmlDeviceGetHandleByIndex_v2`, `nvmlDeviceGetUtilizationRates`,
  `nvmlDeviceGetMemoryInfo`, `nvmlDeviceGetTemperature` (sensor `NVML_TEMPERATURE_GPU`) and
  `nvmlShutdown`. Use device index 0.
- DLL missing or device count 0 → "no GPU"; `nvmlInit_v2` returning
  `NVML_ERROR_DRIVER_NOT_LOADED` → "driver not found"; a failed per-tick call → "read failed" for
  that Metric only.
- `nvmlDeviceGetMemoryInfo` reports dedicated framebuffer memory only, which matches the VRAM
  definition in `CONTEXT.md`.
- **Rationale**: Constitution technical constraints name NVML; runtime loading makes "no GPU" a
  normal state. No `nvml-wrapper` crate (Constitution VI): seven functions.
- **Risk**: `nvml.dll` may add several MB of private memory. Measured as soon as the sampler
  exists (task in Phase 2); if the Panel-closed budget is at risk, NVML is the first thing to
  revisit.

## R10. Sampling thread and data flow

- **Decision**: One `sampler` thread, created at startup, owns every source and all state that
  changes per tick (Sparkline histories, Warning Levels with hysteresis, Status Dot). Every second
  (`thread::sleep` until the next whole-second deadline; Constitution VII) it builds an immutable
  `Snapshot` and hands it to the UI thread with `slint::invoke_from_event_loop`.
- The UI thread stores the latest Snapshot; it updates the Tray Icon only when the Status Dot or
  tooltip changes, and updates Panel properties only while the Panel is visible.
- **Gaps for sleep (FR-021)**: each tick compares wall-clock time to the previous tick; if more
  than 1.5 s passed, it pushes `min(elapsed − 1, 60)` gap samples before the new one.
- **Quit**: an `AtomicBool` stop flag; the sampler checks it every tick and exits; the UI thread
  joins it with a 2 s limit, then removes the Tray Icon and quits the event loop.
- **Rationale**: Sensor reads off the UI thread (Constitution VII); a single owner of mutable
  state means no locks; a whole-Snapshot handoff keeps the UI code simple.

## R11. Sparkline rendering

- **Decision**: A `Sparkline` Slint component draws a `Path` whose `commands` string is built by a
  pure function `sparkline::path(&History, min, max) -> String` in a 60 × 20 viewbox: one
  `M x y L …` run per contiguous block of samples, so gaps break the line. Scales: 0–100 for
  percentages; for temperatures, 20–100 °C.
- **Rationale**: Slint's `Path` with a commands string is cheap on the software renderer, and the
  string builder is unit-testable.

## R12. Theme and DPI in the Panel

- **Decision**: Colors are tokens in `ui/theme.slint`, switched on
  `Palette.color-scheme == ColorScheme.dark`; Slint's winit backend follows the Windows app theme
  and per-monitor DPI. Warning colors: amber `#F5A623`/`#B26B00` (dark/light text) and red
  `#E5484D`/`#C62828`, each checked for ≥ 4.5:1 contrast on the Panel background.
- Font: Segoe UI Variable via Slint's default system font lookup; no bundled font.
