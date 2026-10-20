# Computer Stats

A lightweight Windows 11 tray app that shows the PC's Metrics (CPU Usage, CPU Temperature, GPU
Usage, GPU Temperature, VRAM, Memory Usage, Disk Space) in a Panel above its Tray Icon. Terms are
defined in [`CONTEXT.md`](CONTEXT.md); principles in
[`.specify/memory/constitution.md`](.specify/memory/constitution.md); decisions in
[`docs/adr/`](docs/adr/).

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

CPU Temperature needs the PawnIO driver (Official edition) and `AMDFamily17.bin` copied next to the
executable (see `specs/001-cpu-temp-spike/quickstart.md`). GPU Metrics need an NVIDIA GPU and
driver. Without them, those Metrics show `—` with a reason and everything else keeps working.

Resource budgets (Constitution I):

```powershell
.\scripts\measure.ps1 -Minutes 10            # Panel closed
.\scripts\measure.ps1 -Minutes 10 -PanelOpen # Panel open
```

## Third-party

This application is built with [Slint](https://slint.dev), used under the
[Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0](https://slint.dev/terms-and-conditions#royalty-free).

[![Made with Slint](https://slint.dev/logo/slint-logo-small-light.svg)](https://slint.dev)
