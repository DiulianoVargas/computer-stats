# Quickstart: Fan Speeds

## Prerequisites

- PawnIO installed; `LpcIO.bin` next to the exe (copied by `scripts/package.ps1`, or from
  `target\package\modules-extract\` for a dev build).
- `cargo build --release`; run `target\release\computer-stats.exe` (elevated by its manifest).
- HWiNFO (sensors-only mode) as the reference for RPM readings.
- F4–F6 need a supported chip; since the FR-006 revision the developer's PC qualifies (the app clears its lock).

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
| G1 | Gigabyte B550M AORUS ELITE (ITE IT8689E): open the Panel | Fans lists each fan iCUE shows (e.g. 2 fans), RPM within 5 % of iCUE / HWiNFO |
| G2 | On G1's PC, run iCUE at the same time for 5 min | Both keep showing the same fans; no wrong spikes in either (R5) |

## Results

| # | Result | Notes |
|---|--------|-------|
| F1 | Pass (2026-10-20) | Release exe, elevated: GPU Fan `829 RPM` (spike S2: 830 RPM); Fans `— locked by firmware` (0.4.0, before the FR-006 revision) |
| F2 | Pending | Needs a GPU load next to HWiNFO |
| F3 | Pass | Widget grew 206 → 214 px: one Fan row in the GPU group, none in the CPU group; checked on screen by the user |
| F4 | Pass in part (after FR-006 revision) | Developer's PC, lock cleared by the app: two Fans shown, about 700 and 3200 RPM. Not yet compared with HWiNFO; header names not yet mapped |
| F5–F6 | Pending | |
| G1 (0.5.0) | Fail | Gigabyte B550M AORUS ELITE: Fans `— unsupported motherboard`; GPU Fan `— not reported` with the RTX 5060 Ti fan stopped at idle (expected, research R1). 0.5.1 logs the detection (research R10) |
| G1–G2 | Pending | Rerun with 0.5.1 and the `crash.log` line |
| F4 (regression after ITE support) | Pass | Developer's PC: the same two fans still shown after the detection gained the ITE path |
| F7–F9 | Pending | Each reason is covered by unit tests (`metric::Reason`, `domain::fan`); manual runs pending |
| F10 | See [measurements.md](measurements.md) | |
