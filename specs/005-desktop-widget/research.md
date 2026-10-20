# Research: Desktop Widget

## R1. Drawing over the wallpaper with the software renderer

- **Decision**: the Desktop Widget is a Win32 **layered window** we own (`WS_EX_LAYERED`, in
  `src/platform/desktop_window.rs`), presented with `UpdateLayeredWindow(ULW_ALPHA)` from a
  premultiplied BGRA DIB section. Slint draws the content in a separate `WidgetWindow` component
  that is never shown: once per Snapshot the app sets its properties and calls
  `slint::Window::take_snapshot()`, converts the RGBA result to premultiplied BGRA and presents it.
- **Why it works**: `i-slint-renderer-software` 1.18.1 `take_snapshot` renders into a
  `PremultipliedRgbaColor` buffer whose background is `(0,0,0,0)`, so unpainted pixels stay fully
  transparent and anti-aliased text edges keep partial alpha. `UpdateLayeredWindow` with per-pixel
  alpha is supported on every Windows version the app targets (Windows 10 1809+) and blends with
  the wallpaper through DWM, with no GPU renderer (Constitution V).
- **Alternatives considered**:
  - *winit `with_transparent(true)` on a shown Slint window*: softbuffer 0.4.8's Win32 backend
    presents with `BitBlt(SRCCOPY)` into the window DC; whether DWM honours that alpha depends on
    the blur-behind trick and is not documented. It would also leave the window procedure in
    winit's hands, so hit-testing, z-order pinning and no-activate behaviour (R2, R3) would need
    subclassing a window we do not own.
  - *Colour-key transparency (`LWA_COLORKEY`)*: no partial alpha, so anti-aliased text gets a
    coloured fringe on any wallpaper. Rejected.
  - *GPU renderer (Skia/FemtoVG) with a transparent surface*: violates Constitution V and would
    show up in our own GPU Usage/VRAM readings.
- **To verify in spike S1 (first implementation task)**: `take_snapshot` works on a component that
  was never shown, at a scale factor we choose (R6), and costs < 2 ms per frame at 150 %. Fallback
  if a never-shown window cannot be snapshotted: show the Slint window **cloaked**
  (`DWMWA_CLOAK`) on the same monitor as the layered window. It stays invisible and still renders.

## R2. Hit-testing and dragging

- **Decision**: the Slint background is `#00000001` (alpha 1/255), so the whole rectangle has
  non-zero alpha and receives the mouse (pixels with alpha 0 are click-through in a layered
  window). That alpha is invisible to the eye. `WM_NCHITTEST` returns `HTCAPTION` everywhere, so
  Windows does the drag natively (FR-015). `WM_MOUSEACTIVATE` returns `MA_NOACTIVATE`, and the
  window has `WS_EX_NOACTIVATE`, so clicking or dragging never takes focus (FR-005).
  `WM_EXITSIZEMOVE` reports the final position. The app clamps it to the work area (R6) and saves
  it (R5).
- **Rationale**: no custom drag code, and a drag looks like any window drag.
- **Alternatives considered**: a Slint `TouchArea` drag (the Slint window is never shown, so it
  cannot receive input); `WM_LBUTTONDOWN` + `SetCapture` + manual moves (more code, same result).

## R3. Staying behind every window

- **Decision**: create with `WS_POPUP`, `WS_EX_TOOLWINDOW` (no taskbar button, not in Alt+Tab;
  FR-005) and no owner. Place with `SetWindowPos(HWND_BOTTOM, SWP_NOACTIVATE)`. Handle
  `WM_WINDOWPOSCHANGING` by forcing `hwndInsertAfter = HWND_BOTTOM` (unless R4 has raised it), so
  clicks, drags or other apps cannot bring it forward (FR-004, SC-006).
- **Why no owner**: the Panel's winit hook makes Slint windows owned by the tray window. An owned
  window always sits above its owner, which would fight `HWND_BOTTOM`. The Desktop Widget window
  is created by our own code, so the hook does not apply to it.
- **Alternatives considered**: `HWND_BOTTOM` once, without the `WM_WINDOWPOSCHANGING` guard (any
  activation brings it forward again).

## R4. Staying visible on "Show desktop" (Win+D)

- **Decision**: an out-of-context `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` on the UI thread.
  When the foreground window becomes the desktop (class `WorkerW` or `Progman`, owned by the shell
  process from `GetShellWindow`), the Desktop Widget is raised with `SetWindowPos(HWND_TOP,
  SWP_NOACTIVATE | SWP_SHOWWINDOW)`, so it shows over the desktop. When any other window becomes
  foreground, it goes back to `HWND_BOTTOM`. This is the approach Rainmeter's "On desktop" mode
  uses.
- **Fallback (FR-014)**: if Win+D still hides it on some Windows build (detected in spike S2 on
  Windows 10 22H2 and Windows 11 24H2+), the hook stays harmless. The widget then hides with the
  other windows and returns with them, with no error and no lost position, which the spec accepts.
- **Rationale**: no reparenting into Explorer's windows. An out-of-context WinEvent hook injects
  nothing into other processes. Events are delivered to our message loop, and winit already pumps
  it.
- **Alternatives considered**: reparent into Explorer's `WorkerW` (`SetParent` after sending
  `0x052C` to `Progman`). Rejected because:
  1. Cross-process parenting attaches our input queue to Explorer's, so a hang in either freezes
     both.
  2. An elevated (high integrity) child inside a medium-integrity Explorer window runs into UIPI
     message filtering.
  3. Windows 11 24H2 changed the `WorkerW` layout, which breaks the known recipe.

## R5. Remembering on/off and position

- **Decision**: registry values under `HKCU\Software\Computer Stats` (`Win32_System_Registry` is
  already enabled):
  - `DesktopWidget` (DWORD, 0/1)
  - `DesktopWidgetX` and `DesktopWidgetY` (DWORD, two's-complement i32): top-left in physical
    virtual-screen pixels.

  They are read once at start and written when the widget is turned on or off and at the end of a
  drag. A missing or unreadable value means off, with no position (spec edge case). The uninstaller
  deletes the key (`installer/computer-stats.iss`, `[Registry]` with `uninsdeletekey`).
- **Rationale**: the app has no settings file today, and Start with Windows lives in Task
  Scheduler. A single registry key is the smallest store and is per user, as the spec requires.
  The app is elevated as the same signed-in user, so the key is in the same HKCU.
- **Alternatives considered**: a file in `%APPDATA%` (needs a format and parsing for three
  numbers); Task Scheduler task arguments (wrong place).

## R6. Position, monitors and DPI

- **Decision**: every position is kept in physical pixels, as `domain::anchor` already does.
  - **Scale**: the widget's size is its logical size (from `WidgetWindow` preferred size) ×
    `shell::scale_for(widget rect)`.
  - **Rendering at a scale**: the app tells the Slint window its scale by dispatching
    `slint::platform::WindowEvent::ScaleFactorChanged` (spike S1 confirms this works on a hidden
    window).
  - **Clamping**: a new pure `domain::widget_place::clamp(rect, work_area)` keeps the rectangle
    fully inside `shell::work_area_for(rect)`, the nearest monitor's work area, so it never sits
    under the taskbar. Nearest-monitor is what moves a widget from a disconnected monitor onto a
    connected one (FR-008).
  - **First run**: the top-right of the primary monitor's work area, inset by 24 logical px.
  - **When to re-check**: at start; after a drag; on `WM_DPICHANGED` (re-render at the new scale,
    keep the top-left, then clamp); on `WM_DISPLAYCHANGE` / `WM_SETTINGCHANGE` (SPI_SETWORKAREA).
- **Rationale**: the app manifest is already PerMonitorV2. Physical pixels make SC-002 ("within
  1 px") a direct comparison.

## R7. Readable over any wallpaper

- **Decision**: no card behind the content (FR-010). Each Text gets a **halo**, four copies offset
  by ±1 px drawn first in a contrasting colour at ~70 % alpha. Each Sparkline is drawn twice, a
  3 px halo stroke under the 1.5 px line. In dark mode the content is light with a dark halo; in
  light mode it is dark with a light halo. Warning Level colours use the Panel's `Theme.warning` /
  `Theme.critical` of the active mode.
- **Theme source**: the widget gets an explicit `dark` property from Rust, read from
  `HKCU\…\Themes\Personalize\AppsUseLightTheme`. The never-shown Slint window has no winit window
  for `Palette.color-scheme` to follow. It is re-read on the tray's existing `AppearanceChanged`
  event (US3).
- **Rationale**: offset copies work in the software renderer for any font. Slint text `stroke` and
  `drop-shadow` on Text are not reliably supported by the software renderer in 1.18.
- **Alternatives considered**: a semi-transparent backplate (that was the rejected answer to Q3);
  measuring wallpaper luminance under the widget (complex, and the wallpaper changes in
  slideshows).

## R8. Building the second Slint component

- **Decision**: `ui/widget.slint` defines `WidgetWindow`. `ui/panel.slint` re-exports it
  (`export { WidgetWindow } from "widget.slint";`), so `build.rs` keeps compiling a single entry
  file, and Slint generates every exported Window component.
- **Reuse**: `MetricRowData` and `ui::metric_row()` are reused as they are. The widget has its own
  `HaloMetricRow`, because `MetricRow` uses Panel colours with no halo.

## R9. Cost

- **Per frame** (estimate, to be confirmed by `scripts/measure.ps1`): ~280×190 logical px, which
  at 150 % is ~420×285 = 120 k px.
  - One full software render.
  - Two conversions over 480 KB.
  - One `UpdateLayeredWindow`.
- **Expected**: < 2 ms per second, so ≈ 0.1–0.2 % of one core, which is ≈ 0.02 % of an 8-thread
  CPU.
- **Memory**: the Slint component, a reusable DIB section and per-frame snapshot buffers come to
  ≈ 1–2 MB, far below the 45 MB budget.
- **Widget off**: nothing is created and nothing runs.
