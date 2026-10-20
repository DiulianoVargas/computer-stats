# Quickstart: Desktop Widget

## Prerequisites

- Windows 11 (and one Windows 10 22H2 machine or VM for W6/W7); two monitors with different
  scaling for W10.
- `cargo build --release`; run `target\release\computer-stats.exe` (elevated by its manifest).
- Reset state between runs: `reg delete "HKCU\Software\Computer Stats" /f`.

## Automated

```powershell
cargo fmt --check; cargo clippy -- -D warnings; cargo deny check; cargo test
```

`cargo test` covers `domain::widget_place` (see [data-model.md](data-model.md#placement-pure-srcdomainwidget_placers)).

## Manual scenarios

| # | Steps | Expected |
|---|-------|----------|
| W1 | Fresh state; right-click the Tray Icon | "Show desktop widget" unchecked; nothing on the desktop |
| W2 | Choose "Show desktop widget" | Widget at top-right of primary monitor; item checked; Panel still opens on left-click |
| W3 | Run a CPU load (e.g. `while($true){}` in a PowerShell window) | Widget CPU Usage and Sparkline rise within 2 s and match the Panel |
| W4 | Click the widget, then open Notepad over it | No focus change on click; Notepad covers it; it never covers Notepad (repeat 20× — SC-006) |
| W5 | Alt+Tab; look at the taskbar | Widget not listed |
| W6 | Win+D | Widget remains visible over the desktop (Windows 10 and 11); Win+D again restores windows above it |
| W7 | Drag the widget halfway off the right edge / under the taskbar | It snaps fully into the work area |
| W8 | Drag it to the second monitor's top-right; quit; start the app | Same position (±1 px) |
| W9 | Turn it off; quit; start | Stays off |
| W10 | Move it between a 100 % and 150 % monitor | Re-scales; text sharp |
| W11 | Leave it on the second monitor; disconnect that monitor | Within 2 s it is fully visible on the remaining monitor |
| W12 | Switch Windows between light and dark mode | Colours follow within a few seconds |
| W13 | Plain white wallpaper, plain black wallpaper, a busy photo; both modes | All values readable (SC-008) |
| W14 | Without PawnIO / without a GPU | Those rows show `—`, others update |
| W15 | Explorer restart (`taskkill /f /im explorer.exe; start explorer`) | Widget still shown at its place; Win+D still works |
| W16 | `scripts/measure.ps1` with the widget on, Panel closed, 10 min | ≤ 30 MB private, < 0.5 % CPU (SC-004); record in `measurements.md` |
