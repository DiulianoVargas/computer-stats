# Tasks: Fan Speeds

**Input**: Design documents from `/specs/006-fan-speeds/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ui.md, quickstart.md

**Tests**: domain rules get inline `#[cfg(test)]` unit tests, as in every `src/domain/` file
(Constitution: domain is unit-tested). Hardware paths are validated manually (quickstart F1–F10).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: User story (US1 motherboard fans, US2 GPU Fan, US3 Desktop Widget)

---

## Phase 1: Setup

- [X] T001 Add "Fan Speed", "Fan", "GPU Fan", "CPU Fan" and "Fans" (the Metrics View section) to `CONTEXT.md` under Metrics, with *Avoid* terms (fan RPM, cooler, blower) — before any code uses them (Constitution IX)
- [X] T002 [P] Amend `.specify/memory/constitution.md` 1.2.0 → 1.2.1 (PATCH): add "PawnIO `LpcIO` module (motherboard Fan Speeds)" and "D3DKMT perf data (GPU Fan)" to Technical Constraints → Data sources; add a Sync Impact Report entry
- [X] T003 [P] Pin `LpcIO.bin` in `scripts/package.ps1` (`$ModuleSha256['LpcIO.bin'] = 'B3896A1CAB0D808FCA31FE2EBCAE045D59DAC690DA87B17C858BB8DA357EB45E'`, copied to `module\` and the portable zip like `IntelMSR.bin`) and add it to `[Files]` in `installer/computer-stats.iss`

## Phase 2: Foundational (blocks all stories)

- [X] T004 Add `Reason::{NotReported, UnsupportedMotherboard, LockedByFirmware}` with messages "not reported", "unsupported motherboard", "locked by firmware" in `src/domain/metric.rs`
- [X] T005 Add `MetricKind::GpuFan` after `GpuTemperature` in `src/domain/metric.rs` (`ALL` becomes 7, `display_name` "GPU Fan", `is_fan()`); `Reading::validated` rule for fans: "0–10 000, else `ReadFailed`", not clamped; tests for the new order and the range
- [X] T006 Change every fixed `6` metric array to `MetricKind::ALL.len()` in `src/sampler.rs` (`levels`, `histories`) and `src/snapshot.rs` (`metrics`); `thresholds(GpuFan)` returns `None` (always Normal, FR-007)
- [X] T007 [P] Add `format::rpm(v) -> "{rounded} RPM"` (no thousands separator, e.g. `1240 RPM`) with tests in `src/domain/format.rs`
- [X] T008 [P] Add the fan Sparkline scale "fixed 0–3000 RPM, clamped" to `sparkline::scale` (via `MetricKind::is_fan`) with a test in `src/domain/sparkline.rs`
- [X] T009 Map `GpuFan` in `metric_row` (`src/ui.rs`): label "Fan", value `format::rpm`; check `status_dot::compute` and `tooltip` ignore Normal-only Metrics (no change expected) in `src/domain/status_dot.rs`

**Checkpoint**: `cargo test` passes with 7 Metrics; the GPU Fan row exists and reads `— read failed` until US2.

## Phase 3: User Story 1 — Motherboard fans in the Panel (P1) 🎯 MVP

**Goal**: a Fans section with one row per connected motherboard fan, or one `— reason` row.

**Independent test**: quickstart F1 (developer PC: `— locked by firmware`), F4–F8.

- [X] T010 [P] [US1] Create `src/domain/fan.rs` (+ `pub mod fan;` in `src/domain/mod.rs`): `FanChip { name, channels }` and `fan_chip(id, revision)` with the six rows of research R3 (6 channels for NCT6796D, 7 for the others; count registers `0x4B0 0x4B2 0x4B4 0x4B6 0x4B8 0x4BA 0x4CC`); `is_valid_base(addr)` = "≥ 0x100 and `addr & 0xF007 == 0`"; tests for each chip, an unknown chip and bases
- [X] T011 [US1] In `src/domain/fan.rs`: `FanSample::{Rpm(f32), Stopped, Invalid}` and `fan_speed(high, low)`: `count = (high << 5) | (low & 0x1F)`; "`≥ 0x1FFF` → Stopped; `0x15 ..< 0x1FFF` → Rpm(1 350 000 / count); `< 0x15` → Invalid; Rpm above 10 000 → Invalid"; tests at each boundary
- [X] T012 [US1] In `src/domain/fan.rs`: `FanList` — per channel `seen` + `History`; `update(samples, gaps)` marks a channel seen on its first `Rpm`, keeps it listed afterwards ("Stopped" → 0 RPM value, "Invalid" → gap), never lists unseen channels; `labels_for(board) -> Option<&[&str]>` (empty table, research R6) and "Fan {n}" fallback; `cpu_fan` flag for a label "CPU fan"; tests: unseen hidden, seen-then-stopped stays at 0, gaps, labels fallback
- [X] T013 [US1] Refactor `src/platform/pawnio.rs`: make `PawnIo`, `PawnIo::open` and `is_elevated` `pub(super)`; add `PawnIo::call(function, input: &[u64], output: &mut [u64]) -> Result<(), Reason>` and rebuild `execute` on it; `CpuTemperatureSource` behaviour unchanged
- [X] T014 [US1] Create `src/platform/superio.rs` (+ `pub mod superio;` in `src/platform/mod.rs`): `IsaBusMutex` (create-or-open `Global\Access_ISABUS.HTP.Method`, `lock(10 ms) -> Option<Guard>` releasing on drop, abandoned counts as acquired); `board_product()` from `HKLM\HARDWARE\DESCRIPTION\System\BIOS\BaseBoardProduct`
- [X] T015 [US1] In `src/platform/superio.rs`: `FanSource::open()` — elevation (`NotElevated`), `LpcIO.bin` next to the exe via `PawnIo::open` (`ModuleNotFound` / `DriverNotFound`), then `detect()` per research R2: under the mutex, slot 0 then 1: enter `0x87 0x87`, read ID `0x20` / rev `0x21`, on a `fan_chip` match `ioctl_find_bars`, select LDN `0x0B`, base `0x60` twice 1 ms apart (equal and `is_valid_base`), lock = `0x28` bit 4; always exit with `0xAA`. Outcomes: `Ready { base, chip }`, `Locked { next_check: now + 60 s }` (`LockedByFirmware`), `UnsupportedMotherboard`. Never write `0x28`, `0x30` or any HWM register other than bank select `0x4E` (FR-006)
- [X] T016 [US1] In `src/platform/superio.rs`: `FanSource::read(&mut self) -> Result<Vec<FanSample>, Reason>` — Ready: under the mutex (busy → every channel `Invalid`), per channel read high/low via `base+5`/`base+6` (`0x4E`, bank, register, data) and `fan_speed`; every channel `0xFF/0xFF` → back to detection (lock set again). Locked: re-detect when `next_check` passes, else `Err(LockedByFirmware)`. No panics; port errors → `ReadFailed`
- [X] T017 [US1] Add `FansState::{Unavailable(Reason), Fans(Vec<FanState>)}` and `FanState { label, reading, history, cpu_fan }` to `src/snapshot.rs` (`Snapshot.fans`; `Snapshot::unavailable` sets `Unavailable(reason)`)
- [X] T018 [US1] Wire `FanSource` and `FanList` into `src/sampler.rs`: `fans: Result<FanSource, Reason>` opened in `Sampler::new` on the sampler thread; each tick `read()` → `FanList::update` (with `missed_seconds` gaps) → `FansState`; open/read errors → `Unavailable(reason)`
- [X] T019 [P] [US1] Add `in property <[MetricRowData]> fans;` and `in property <string> fans-reason;` to `ui/metrics-view.slint` and `ui/panel.slint` (pass-through); a "Fans" section after Memory, before Drives: one `MetricRow` per fan; when `fans-reason != ""` a single muted row "Fans  — reason"; hidden when both are empty (contracts/ui.md)
- [X] T020 [US1] In `src/ui.rs` `push_snapshot`: build fan rows (`label`, `format::rpm`, fan Sparkline scale, level 0) and the reason string from `latest.fans`; the Panel re-anchors on height change as it does for Drives

**Checkpoint**: on the developer's PC the Fans section reads `— locked by firmware` (F1); renaming `LpcIO.bin` gives `— module not found` (F7).

## Phase 4: User Story 2 — GPU Fan (P2)

**Goal**: GPU Fan row with RPM and Sparkline in the GPU group.

**Independent test**: quickstart F1, F2, F9.

- [X] T021 [P] [US2] In `src/domain/fan.rs`: `GpuFanState { reported }` and `next(rpm: u32) -> Reading` — "`FanRPM > 0` sets `reported`; while `!reported` the reading is `Unavailable(NotReported)`; after, `Reading::of(MetricKind::GpuFan, rpm)`"; tests
- [X] T022 [US2] In `src/platform/gpu.rs`: add `fan: Reading` to `GpuReadings` (`unavailable` included), read `FanRPM` from the same `perf_data()` call as Temperature (one query per tick), keep a `GpuFanState` in `GpuSource`; perf data failure → `ReadFailed`
- [X] T023 [US2] Put `gpu.fan` at `MetricKind::GpuFan`'s index in the readings array in `src/sampler.rs`
- [X] T024 [US2] Add `in property <MetricRowData> gpu-fan;` to `ui/metrics-view.slint` (row between GPU Temperature and VRAM) and `ui/panel.slint`; set it in `src/ui.rs` `push_snapshot`

**Checkpoint**: F1/F2 on the developer's PC: GPU Fan within 5 % of HWiNFO.

## Phase 5: User Story 3 — Fans on the Desktop Widget (P3)

**Goal**: Fan rows in the widget's CPU group (CPU Fan, known boards only) and GPU group.

**Independent test**: quickstart F3.

- [X] T025 [P] [US3] Add `in property <MetricRowData> gpu-fan;`, `in property <MetricRowData> cpu-fan;` and `in property <bool> has-cpu-fan;` to `ui/widget.slint`; Fan row in the GPU group after Temperature; CPU Fan row in the CPU group only when `has-cpu-fan`
- [X] T026 [US3] In `src/desktop_widget.rs` `show`/update: set `gpu-fan` from `metric_row` (shows `— no GPU` without a dedicated GPU, like the other GPU rows); set `cpu-fan`/`has-cpu-fan` from the `FanState` with `cpu_fan == true` (none today, research R6)

## Phase 6: Polish

- [X] T027 [P] README: Fan speeds in the feature list (GPU fan; motherboard fans on supported, unlocked Nuvoton chips; `— locked by firmware` explained in troubleshooting) and the 006 row in the feature table in `README.md`
- [X] T028 Gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo deny check`, `cargo test`
- [X] T029 Validate quickstart F1–F3 and F7–F9 on the developer's PC (elevated run via the release exe) and record results in `specs/006-fan-speeds/quickstart.md`; F4–F6 marked "pending: needs an unlocked Nuvoton PC"
- [X] T030 Measure with `scripts/measure.ps1` (F10) and record in `specs/006-fan-speeds/measurements.md`; budgets as plan.md Constraints

## Dependencies & Execution Order

- Setup (T001–T003) → Foundational (T004–T009) → US1, US2, US3.
- US1 and US2 are independent after Phase 2 (different files except `src/sampler.rs`/`src/ui.rs`,
  edited in sequence). US3 needs T017 (`FanState`) and T022 (`GpuFan`).
- Within US1: T010 → T011 → T012 (same file); T013 → T014 → T015 → T016; T017 → T018 → T020;
  T019 any time after T017.

## Parallel Example

```text
T002 constitution  |  T003 packaging  |  T007 format::rpm  |  T008 sparkline scale
T010 domain::fan chips  |  T013 PawnIo refactor  |  T019 metrics-view.slint
```

## Implementation Strategy

1. Phases 1–2: the vocabulary, the shipped module and a 7th Metric.
2. US1 (MVP): motherboard fans end to end, including the locked and unsupported paths that the
   developer's PC exercises.
3. US2: GPU Fan, the part fully verifiable on the developer's PC.
4. US3, then polish, gates and measurement before any release.

## Notes

- Never clear the I/O space lock or enable a disabled device (spec FR-006; user decision).
- `examples/fan_spike.rs` stays as a diagnostic for new boards.
