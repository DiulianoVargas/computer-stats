# Data Model: Desktop Widget

All positions and sizes are physical pixels in virtual-screen coordinates (`domain::anchor`
types).

## WidgetSettings (`src/domain/widget_place.rs`)

| Field | Type | Meaning |
|-------|------|---------|
| `shown` | `bool` | The Desktop Widget is on. Default `false` |
| `position` | `Option<Point>` | Top-left last placed by the user; `None` means never placed |

- Loaded once at start from `HKCU\Software\Computer Stats` (research R5); any unreadable value
  means `shown: false, position: None`.
- Saved when `shown` changes and at the end of a drag; a failed write is ignored (FR-013).

## Placement (pure, `src/domain/widget_place.rs`)

- `default_position(work_area: Rect, size: Size, inset: i32) -> Point`: top-right corner of the
  primary monitor's work area, `inset` = 24 logical px × scale.
- `clamp(top_left: Point, size: Size, work_area: Rect) -> Point`: the closest position that keeps
  the whole rectangle inside `work_area`. If `size` is larger than the work area, top-left is
  pinned to the work area's top-left.
- Callers pass the work area of the monitor nearest to the widget's rectangle, which is how a
  widget on a disconnected monitor lands on a connected one (FR-008).

Unit tests: inside → unchanged; off each edge → pulled in; fully off-screen (old monitor) → on
the nearest work area; under the taskbar → above it; larger than the work area → pinned.

## DesktopWidget state (`src/desktop_widget.rs`, UI thread)

```text
        Tray Menu "Show desktop widget" (unchecked)
 Off ─────────────────────────────────────────────────▶ On(Behind)
  ▲                                                     │   ▲
  │ Tray Menu (checked) / window creation failed        │   │ another window
  │                                                     ▼   │ becomes foreground
  └─────────────────────────────────────────────── On(OverDesktop)
                                       desktop became foreground (Win+D, R4)
```

- **Off**: no Slint `WidgetWindow`, no layered window, no WinEvent hook, no per-frame work.
- **On(Behind)**: shown at `HWND_BOTTOM`; repainted on every Snapshot.
- **On(OverDesktop)**: raised to `HWND_TOPMOST` without activation while Show desktop holds the
  desktop (topmost `WorkerW`/`Progman`) in front.
- Turning off from either On state frees the window, the hook and the buffers.
- If creating the window fails, the state stays Off, the menu stays unchecked and the failure goes
  to the crash log (FR-013).

## WidgetWindow properties (`ui/widget.slint`)

| Property | Type | Source |
|----------|------|--------|
| `cpu-usage`, `cpu-temperature`, `gpu-usage`, `gpu-temperature`, `memory-usage` | `MetricRowData` | `ui::metric_row()` on the Snapshot, same as the Panel (FR-002, SC-005) |
| `dark` | `bool` | `theme::apps_use_light_theme()` negated (research R7) |
| `preferred-width/height` | out, logical | Used ×scale to size the layered window |

## Events from the platform window (`platform::desktop_window::WidgetEvent`)

Show desktop (R4) is handled inside the platform window: the foreground hook raises or lowers it
directly, with no event to the app.

| Event | When | App reaction |
|-------|------|--------------|
| `Moved(Point)` | `WM_EXITSIZEMOVE` | clamp, reposition if needed, save |
| `DpiChanged(f32)` | `WM_DPICHANGED` | re-render at new scale, clamp |
| `DisplayChanged` | `WM_DISPLAYCHANGE`, work-area change | clamp |
