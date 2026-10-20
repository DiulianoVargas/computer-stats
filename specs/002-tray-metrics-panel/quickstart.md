# Quickstart: Tray Icon and Metrics Panel

## Prerequisites

1. Rust stable (1.90+): `winget install Rustlang.Rustup`.
2. `cargo-deny`: `cargo install cargo-deny`.
3. For CPU Temperature: PawnIO **Official** edition installed, and `AMDFamily17.bin` (from the
   PawnIO.Modules `release_X_Y_Z.zip` asset) copied next to the built executable, as in
   spike 001.
4. An NVIDIA GPU with its driver, for the GPU Metrics (optional: without it they show `— no GPU`).

## Build and unit tests (no elevation needed)

```powershell
cargo fmt --check
cargo clippy -- -D warnings
cargo deny check
cargo test
cargo build --release
```

Expected: all pass. Unit tests cover Warning Level thresholds and hysteresis, Status Dot rules,
History ring buffer, Sparkline path building, value formatting, Panel anchoring, Tray Icon art
and the task XML.

## Run

From an **elevated** terminal (the manifest asks for elevation, see research R6):

```powershell
Copy-Item path\to\AMDFamily17.bin target\release\
.\target\release\computer-stats.exe
```

Budget check (Constitution I):

```powershell
.\scripts\measure.ps1 -Minutes 10            # Panel closed
.\scripts\measure.ps1 -Minutes 10 -PanelOpen # Panel open (keep it open during the run)
```

## Scenario validation

| # | Scenario | How | Expected |
|---|----------|-----|----------|
| Q1 | Open/close (US1) | Left-click the Tray Icon; click the desktop; open again, press Esc; open again, click the Tray Icon | Panel opens above the icon each time and closes each way; no taskbar button, not in Alt+Tab |
| Q2 | Live values (US1, SC-004) | Open the Panel next to Task Manager for 30 s | Values change every second; CPU Usage and Memory Usage within 5 points of Task Manager |
| Q3 | Quit (US1) | Right-click → Quit | Tray Icon disappears; process ends |
| Q4 | Warning colors (US2, SC-005) | Fill memory past 80% (e.g. a RAM-hungry tool) | Memory Usage turns amber and the Status Dot turns amber within 2 s; back to normal after release and dropping below 78% |
| Q5 | Temperature Critical (US2) | All-core load until CPU Temperature ≥ 90 °C, or temporarily lower the threshold in a debug build | Red value and red Status Dot; tooltip names CPU Temperature |
| Q6 | Disk Space rule (US2) | A Drive at 85–94% used | Drive amber in the Panel; Status Dot unaffected |
| Q7 | Sparklines (US3) | Wait 60 s, run a 20 s CPU load, open the Panel | CPU Usage/Temperature Sparklines show the rise and fall with full 60 s history |
| Q8 | Start with Windows (US4) | Check it, sign out and in; then uncheck, sign out and in | App starts elevated with no UAC prompt; then it no longer starts. `schtasks /Query /TN "Computer Stats"` matches |
| Q9 | Degradation (SC-006) | Rename `AMDFamily17.bin`; separately run as a standard user; separately on a PC without an NVIDIA GPU (or rename access via a debug flag) | The affected Metric shows `—` + reason; the rest work; 10 min without crash |
| Q10 | Single instance | Launch the exe again while running | No second Tray Icon; the Panel opens |
| Q11 | Explorer restart | `Stop-Process -Name explorer` (it restarts) | Tray Icon reappears within a few seconds |
| Q12 | Taskbar & DPI | Move the taskbar to the top (or use a second monitor at 150%) | Panel anchors next to the icon, fully on screen, sharp text |
| Q13 | Theme | Switch Windows between light and dark | Panel and Tray Icon glyph follow the theme; colors stay legible |
| Q14 | Budgets (SC-002, SC-003) | `measure.ps1` closed and open; and one 24 h run | ≤ 30 MB / < 0.5% CPU closed; ≤ 45 MB / < 2% open; < 2 MB growth over 24 h |
| Q15 | Opening speed (SC-001) | Click the Tray Icon repeatedly | Panel appears immediately with values (no empty state) |
