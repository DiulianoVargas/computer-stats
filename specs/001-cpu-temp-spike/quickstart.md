# Quickstart: CPU Temperature Spike

## Prerequisites

1. Rust: `winget install Rustlang.Rustup`, then restart the terminal.
2. PawnIO driver: install from https://pawnio.eu (PawnIO.Setup releases).
3. `AMDFamily17.bin` from the **`release_X_Y_Z.zip` asset** of the PawnIO.Modules GitHub release
   (not "Source code (zip)", which only has `.p` sources), copied to
   `spikes/cpu-temp/target/release/` after building.
   Choose the **Official (signed)** edition in the PawnIO installer; the Unrestricted edition fails
   with error 256 when Code Integrity is enforced.
4. HWiNFO (sensors-only mode) for reference readings.

## Build and unit tests (no hardware or elevation needed)

```powershell
cd spikes/cpu-temp
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build --release
```

Expected: all tests pass. They cover decoding, the plausibility bounds, line formatting and CPU
family detection.

## Scenario validation

| # | Scenario | How | Expected |
|---|----------|-----|----------|
| V1 | Live reading (US1) | Run `cpu-temp-spike.exe` from an **admin** terminal | A °C line every second |
| V2 | Clean exit (US1) | Press Ctrl+C, then run `echo $LASTEXITCODE` | `0` |
| V3 | Not elevated (US3) | Run from a normal terminal for 30 s | `—  (not elevated)` lines, no crash |
| V4 | Driver missing (US3) | Elevated, after temporarily renaming `AMDFamily17.bin` | `—  (driver not found)`, no crash |
| V5 | Accuracy, idle (US2, SC-002) | Elevated with output piped to `idle.log`; HWiNFO logging to CSV; 60 s idle | Every paired sample within ±2 °C |
| V6 | Accuracy, load (US2, SC-002) | Same as V5 during an all-core load such as Cinebench | Within ±2 °C; both curves rise together |
| V7 | Stability (SC-001) | Elevated for 5 min | 300 lines with values, 0 `—` |
| V8 | Memory (SC-004) | `(Get-Process cpu-temp-spike).PrivateMemorySize64 / 1MB` | < 10 |

V3 for "unsupported CPU" can only be checked on an Intel machine. The unit test for family
detection covers it otherwise.

## Record the verdict

Fill in `specs/001-cpu-temp-spike/verdict.md` with the V5–V8 numbers and **Confirmed** or
**Revised**. If revised, write an ADR that supersedes `docs/adr/0002-run-elevated-for-cpu-temperature.md`.
