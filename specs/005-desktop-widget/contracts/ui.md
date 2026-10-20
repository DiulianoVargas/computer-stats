# UI Contract: Desktop Widget

## Tray Menu

```text
✓ Start with Windows
  Show desktop widget        ← new, checkable; ✓ while shown
  ─────────────
  Quit
```

- Choosing it toggles the Desktop Widget. The check mark reflects the actual state: if showing
  fails, it stays unchecked, with no dialog (same rule as Start with Windows).
- The Tray Icon, Status Dot, tooltip and Panel are unchanged (FR-012).

## Desktop Widget layout

Logical size 320 × 184 px, fixed (no resizing). It scales with the monitor.

```text
CPU
  Usage             23 %        ╱╲_╱‾╲
  Temperature      54 °C        ‾‾╲__╱
GPU
  Usage             12 %        __╱╲__
  Temperature      48 °C        ‾‾‾‾‾‾
Memory
  Usage    9.8 / 16.0 GB  61 %  ‾‾‾╲_╱
```

- Same rows, labels, value formats, `—` reasons, Warning Level colours and Sparkline scales as
  the Panel's Metrics View, for exactly these five Metrics (FR-002). No VRAM, no Drives, no App
  Volumes, no Processes (FR-011).
- No background card. Every text and Sparkline has a halo for contrast (research R7):

| Mode | Content | Halo |
|------|---------|------|
| Dark | `#ffffff`, muted `#c8c8c8` | `#000000` at 70 % |
| Light | `#1b1b1b`, muted `#424242` | `#ffffff` at 70 % |

- Warning and Critical use the Panel's colours for the active mode.

## Behaviour

| Action | Result |
|--------|--------|
| Drag anywhere on the rectangle | Moves it; dropped position is clamped into the monitor's work area and saved |
| Click | Nothing; no focus change; does not come to the front |
| Other window opened or focused | It covers the Desktop Widget |
| Win+D / Show desktop | The Desktop Widget stays visible over the desktop (fallback: hides and returns with the windows) |
| Alt+Tab, taskbar | Never listed |
| Windows light/dark switch | Follows within a few seconds |
| Monitor removed / resolution change | Moves onto the nearest connected work area |
| First time shown | Top-right of the primary monitor's work area, 24 px inset |
| App start | Restored as it was at quit (on/off, position) |
