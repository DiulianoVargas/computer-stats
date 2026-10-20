# Quickstart: Fan Speeds

## Prerequisites

- PawnIO installed; `LpcIO.bin` next to the exe (copied by `scripts/package.ps1`, or from
  `target\package\modules-extract\` for a dev build).
- `cargo build --release`; run `target\release\computer-stats.exe` (elevated by its manifest).
- HWiNFO (sensors-only mode) as the reference for RPM readings.
- F4–F6 need a PC whose Nuvoton chip is not locked by its firmware (not the developer's PC).

## Automated

```powershell
cargo fmt --check; cargo clippy -- -D warnings; cargo deny check; cargo test
```

`cargo test` covers `domain::fan` (chip table, count → RPM, `FanList` rules, GPU Fan rule) and the
new `MetricKind` / `Reason` values (see [data-model.md](data-model.md)).

## Spike

```powershell
# Elevated; read-only. Prints chip ID, base, lock bit, 5 s of fan counts and GPU FanRPM.
cargo run --example fan_spike -- target\package\modules-extract\LpcIO.bin
```

## Manual scenarios

| # | Steps | Expected |
|---|-------|----------|
| F1 | Developer's PC: open the Panel | GPU section has `Fan  nnn RPM` within 5 % of HWiNFO's GPU fan; Fans section shows `— locked by firmware` |
| F2 | Run a GPU load (e.g. a game or FurMark) for 1 min | GPU Fan rises with HWiNFO, within 5 % (SC-002) |
| F3 | Turn on the Desktop Widget | GPU group has a Fan row matching the Panel; CPU group has no Fan row (board not known) |
| F4 | Unlocked-chip PC: open the Panel | One row per fan HWiNFO lists, `Fan 1`…, within 5 % idle and under CPU load (SC-001) |
| F5 | On F4's PC, stop a case fan (unplug it or set it to 0 % in the BIOS) | Its row shows `0 RPM` within 2 s and stays (SC-003) |
| F6 | On F4's PC, run HWiNFO at the same time for 5 min | Both show the same fans; no wrong spikes in either (R5) |
| F7 | Rename `LpcIO.bin`, restart the app | Fans: `— module not found`; GPU Fan and every other Metric still work |
| F8 | Run the exe without elevation (`examples/app_unelevated` style) | Fans: `— not elevated`; GPU Fan still works |
| F9 | PC with an integrated GPU only | GPU Fan shows `— no GPU`, like the other GPU rows |
| F10 | `scripts/measure.ps1`, 10 min, Panel closed, widget off | Within budgets (≤ 30 MB, < 0.5 % CPU); exe ≤ 10 MB (SC-005) |

## Results

| # | Result | Notes |
|---|--------|-------|
