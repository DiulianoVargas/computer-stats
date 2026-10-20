# Computer Stats

A lightweight Windows 11 tray app that shows the PC's Metrics (CPU Usage, CPU Temperature, GPU
Usage, GPU Temperature, VRAM, Memory Usage, Disk Space) in a Panel above its Tray Icon. Terms are
defined in [`CONTEXT.md`](CONTEXT.md); principles in
[`.specify/memory/constitution.md`](.specify/memory/constitution.md); decisions in
[`docs/adr/`](docs/adr/).

## Features

- **Tray Icon** with a **Status Dot**: amber or red when a Metric needs attention (hover for which).
- **Panel** (left-click the Tray Icon): every Metric updated each second, colored by Warning
  Level, with a 60-second Sparkline. Click outside, press Esc or click the icon again to close.
- **Tray Menu** (right-click): *Start with Windows* (starts elevated at sign-in, no UAC prompt,
  through a scheduled task named "Computer Stats") and *Quit*.
- Reads GPU Metrics from Windows itself (as Task Manager does; ADR-0003), so it works with any
  dedicated GPU and loads no vendor library. Integrated graphics are ignored.
- **Processes View** (the Panel's second tab): every running App with its icon, Process count,
  CPU and memory, sorted by CPU or memory. **End** turns into **Confirm?** for 3 seconds; a second
  click forcibly ends every Process of that App. Any App can be ended, including Windows' own
  programs and services: ending those can make Windows unstable or restart it.
- Launching it again while it runs opens the existing Panel.

## Build

```powershell
cargo fmt --check
cargo clippy -- -D warnings
cargo deny check
cargo test
cargo build --release
```

## Run

The executable asks for elevation (`highestAvailable`, ADR-0002), so start `cargo run` or the
built exe from an **elevated** terminal during development.

During development, `$env:__COMPAT_LAYER = 'RunAsInvoker'` starts the exe without elevation (CPU
Temperature then shows `— not elevated`). Debug builds accept
`COMPUTER_STATS_TEST_THRESHOLD_OFFSET=<n>` to lower every Warning threshold by `n` and see the
colors without loading the PC, and `COMPUTER_STATS_TEST_VIEW=processes` to open on the Processes
View; they also use their own single-instance lock, so they run next to an installed copy.

CPU Temperature needs the PawnIO driver (Official edition) and `AMDFamily17.bin` copied next to the
executable (see `specs/001-cpu-temp-spike/quickstart.md`). GPU Metrics need a dedicated GPU and its
driver (GPU Temperature only if the driver reports it to Windows, as in Task Manager). Without them, those Metrics show `—` with a reason and everything else keeps working.

Resource budgets (Constitution I):

```powershell
.\scripts\measure.ps1 -Minutes 10            # Panel closed
.\scripts\measure.ps1 -Minutes 10 -PanelOpen # Panel open
```

## Package (installer and portable zip)

```powershell
winget install JRSoftware.InnoSetup   # once
.\scripts\package.ps1
```

Produces in `dist\`:

- `ComputerStats-Setup-<version>.exe`: installs to Program Files with a Start menu shortcut (and
  an optional desktop one), closes a running copy first, and its uninstaller also removes the
  Start with Windows task.
- `ComputerStats-<version>-portable.zip`: unzip and run `computer-stats.exe`.

The exe has no runtime dependencies beyond Windows (static C runtime). Both packages include
`AMDFamily17.bin` (PawnIO.Modules, LGPL-2.1, hash-checked) and the third-party notices; the PawnIO
driver itself must be installed separately for CPU Temperature. The packages are not code-signed,
so Windows SmartScreen shows "Windows protected your PC" the first time (More info → Run anyway).

The app icon is generated in code (`cargo run --example make_icon` rewrites `assets/app.ico`).

## Third-party

This application is built with [Slint](https://slint.dev), used under the
[Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0](https://slint.dev/terms-and-conditions#royalty-free).

[![Made with Slint](https://slint.dev/logo/slint-logo-small-light.svg)](https://slint.dev)
