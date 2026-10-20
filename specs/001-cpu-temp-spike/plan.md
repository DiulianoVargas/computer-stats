# Implementation Plan: CPU Temperature Spike

**Branch**: `001-cpu-temp-spike` | **Date**: 2026-10-19 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-cpu-temp-spike/spec.md`

## Summary

A standalone Rust console program. It checks elevation and CPU support, loads `PawnIOLib.dll` at
runtime, loads the signed `AMDFamily17` module, and once per second reads the SMN register
`0x00059800` and decodes it to the Tctl/Tdie temperature in °C, using the same rules as
LibreHardwareMonitor. Every failure is shown as `—` with a reason. The decoding and formatting
logic is pure and unit-tested. Accuracy against HWiNFO is validated manually (quickstart V5–V6),
and the result is written down as a verdict on ADR-0002.

## Technical Context

**Language/Version**: Rust stable (1.90+), edition 2024

**Primary Dependencies**: `windows` crate only, with minimal features (research R6); runtime: PawnIO driver + `AMDFamily17.bin`

**Storage**: N/A

**Testing**: `cargo test` for the pure logic; manual hardware validation per quickstart.md

**Target Platform**: Windows 11 x64, AMD Ryzen 9 7900 (Zen 4, family 19h model 61h)

**Project Type**: CLI (throwaway spike)

**Performance Goals**: First reading within 2 s, then one reading per second

**Constraints**: < 10 MB private memory (SC-004); never panics (FR-004); elevated

**Scale/Scope**: About 300 lines of code, one binary

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | How |
|-----------|--------|-----|
| I. Resource Budgets | ✅ | Target < 10 MB, within the app's 30 MB budget; release profile per Technical Constraints |
| II. Native Only | ✅ | No web view, runtime or sidecar; `deny.toml` bans enforce it |
| III. Unsafe Isolation | ✅ | All FFI and `CPUID` live in `src/platform.rs`; `main.rs` and `reading.rs` use `#![deny(unsafe_code)]` (crate-level `deny`, with `#[allow]` only on `mod platform`) |
| IV. Elevated Minimal Surface | ✅ | No network crates, no process spawning, and loads only the signed PawnIO module from its fixed install path |
| V. Software Rendering | N/A | No UI |
| VI. Dependency Discipline | ✅ | One crate (`windows`), justified in R6; `cargo deny check` |
| VII. Sampling Discipline | ✅ | 1 s sleep loop on the main thread is acceptable here (no UI thread); no unbounded buffers |
| VIII. Graceful Degradation | ✅ | Every failure maps to a `Reason` (data-model.md); no `unwrap`/`expect` outside tests |
| IX. Code Hygiene | ✅ | fmt + clippy `-D warnings`; names follow CONTEXT.md (`CpuTemperature`) |

**Post-design re-check**: still passing. The design adds no dependencies or capabilities beyond
the table.

## Project Structure

### Documentation (this feature)

```text
specs/001-cpu-temp-spike/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/cli-output.md
├── checklists/requirements.md
├── verdict.md           # written after validation (FR-007)
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
spikes/cpu-temp/
├── Cargo.toml           # windows crate only; release profile: lto, codegen-units=1, panic=abort, strip
├── deny.toml            # bans (web views, network crates), licenses, advisories
└── src/
    ├── main.rs          # tick loop, Ctrl+C flag, printing; #![deny(unsafe_code)]
    ├── reading.rs       # pure: decode_tctl(raw), plausibility, Reason, format_line, is_supported_family
    │                    #       + #[cfg(test)] unit tests
    └── platform.rs      # the only unsafe code: elevation check, CPUID, PawnIOLib loading/calls, GetLocalTime,
                         #   SetConsoleCtrlHandler
```

**Structure Decision**: A standalone crate under `spikes/`, kept separate from the future app
crate so it can be deleted after the verdict. `platform.rs` is designed to be copied into the
app's platform module if the verdict is Confirmed.

## Complexity Tracking

No violations.
