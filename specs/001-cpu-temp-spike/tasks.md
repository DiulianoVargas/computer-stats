---

description: "Task list for the CPU Temperature spike"
---

# Tasks: CPU Temperature Spike

**Input**: Design documents from `/specs/001-cpu-temp-spike/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/cli-output.md, quickstart.md

**Tests**: Requested by the user. Unit tests for the pure logic are written first and must fail
before implementation. Hardware behaviour is validated manually through quickstart.md (V1–V8).

**Organization**: Tasks are grouped by user story. All paths are relative to the repo root; the
crate lives in `spikes/cpu-temp/`.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1 = live reading, US2 = accuracy, US3 = clear failure

---

## Phase 1: Setup

**Purpose**: Crate skeleton and guardrails (Constitution II, IV, VI, IX)

- [X] T001 Create `spikes/cpu-temp/Cargo.toml`: package `cpu-temp-spike`, edition 2024; only dependency `windows` with `default-features = false` and features `Win32_Foundation`, `Win32_System_LibraryLoader`, `Win32_System_Registry`, `Win32_Security`, `Win32_System_Threading`, `Win32_System_Console`, `Win32_System_SystemInformation`; `[profile.release]` with `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`; `[lints.clippy] unwrap_used = "deny"`, `expect_used = "deny"`
- [X] T002 [P] Create `spikes/cpu-temp/deny.toml`: `[bans] deny` list containing `wry`, `webview2-com`, `tauri`, `reqwest`, `hyper`, `ureq`; `multiple-versions = "warn"`; `[licenses] allow = ["MIT", "Apache-2.0", "Unicode-3.0"]`; `[advisories]` defaults
- [X] T003 [P] Create `spikes/cpu-temp/src/main.rs` with `#![deny(unsafe_code)]`, `mod reading;`, `#[allow(unsafe_code)] mod platform;`, and an empty `fn main()`; create empty `spikes/cpu-temp/src/reading.rs` and `spikes/cpu-temp/src/platform.rs`
- [X] T004 [P] Create `spikes/cpu-temp/.gitignore` containing `/target`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The shared types that every story uses (data-model.md)

- [X] T005 In `spikes/cpu-temp/src/reading.rs`, define `pub enum Reason { NotElevated, UnsupportedCpu, DriverNotFound, ReadFailed }` with `fn message(&self) -> &'static str` returning exactly `"not elevated"`, `"unsupported CPU"`, `"driver not found"`, `"read failed"`; define `pub enum Outcome { Celsius(f32), Unavailable(Reason) }`; define `pub struct Clock { h: u8, m: u8, s: u8 }`

**Checkpoint**: The crate compiles (`cargo build`).

---

## Phase 3: User Story 1 - See live CPU Temperature (P1) 🎯 MVP

**Goal**: An elevated run prints `HH:MM:SS  54.3 °C` every second, and Ctrl+C exits with code 0.

**Independent Test**: quickstart V1 + V2.

### Tests for User Story 1 (write first, must fail)

- [X] T006 [P] [US1] In `spikes/cpu-temp/src/reading.rs` `#[cfg(test)] mod tests`, add tests for `decode_tctl(raw: u32) -> f32`: `raw = 432 << 21` gives `54.0`; `raw = (433 << 21)` gives `54.125`; `raw = (824 << 21) | 0x80000` gives `103.0 - 49.0 = 54.0` (range-select offset); `raw = (824 << 21) | 0x30000` gives `54.0` (TJ-select offset); `raw = (824 << 21) | 0x10000` gives `103.0` (TJ partial, no offset)
- [X] T007 [P] [US1] In the same test module, add tests for `format_line(&Clock, &Outcome) -> String`: `Clock{14,2,7}` + `Celsius(54.26)` gives `"14:02:07  54.3 °C"` (one decimal, two-space separator per contracts/cli-output.md); `Clock{9,5,0}` + `Celsius(54.0)` gives `"09:05:00  54.0 °C"`

### Implementation for User Story 1

- [X] T008 [US1] Implement `pub fn decode_tctl(raw: u32) -> f32` in `spikes/cpu-temp/src/reading.rs`: `t = (raw >> 21) as f32 * 0.125`; subtract `49.0` if `raw & 0x80000 != 0 || raw & 0x30000 == 0x30000` (research R3). T006 must pass.
- [X] T009 [US1] Implement `pub fn format_line(clock: &Clock, outcome: &Outcome) -> String` in `spikes/cpu-temp/src/reading.rs`; for `Celsius`, use `format!("{:02}:{:02}:{:02}  {:.1} °C", ...)`. T007 must pass.
- [X] T010 [US1] In `spikes/cpu-temp/src/platform.rs`, implement `pub fn local_clock() -> Clock` via `GetLocalTime`, and `pub fn install_ctrl_handler(flag: &'static AtomicBool)` via `SetConsoleCtrlHandler`, setting the flag on CTRL_C, CTRL_BREAK and CTRL_CLOSE and returning TRUE
- [X] T011 [US1] In `spikes/cpu-temp/src/platform.rs`, implement `pub struct PawnIo` (holds the loaded `HMODULE`, the function pointers and the `HANDLE`) with `pub fn open(module_path: &Path) -> Result<PawnIo, Reason>`. It reads `InstallLocation` from `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO`, falling back to `C:\Program Files\PawnIO`, then `LoadLibraryW("…\\PawnIOLib.dll")`, `GetProcAddress` for `pawnio_open`/`pawnio_load`/`pawnio_execute`/`pawnio_close` with the signatures in research R1, calls `pawnio_open`, reads the module file, and calls `pawnio_load`. Any failure maps to `Reason::DriverNotFound`, except `E_ACCESSDENIED`, which maps to `Reason::NotElevated`. Implement `Drop` calling `pawnio_close`.
- [X] T012 [US1] In `spikes/cpu-temp/src/platform.rs`, implement `pub fn read_smn(&self, offset: u32) -> Result<u32, Reason>`, which calls `pawnio_execute(h, c"ioctl_read_smn", &[offset as u64], 1, &mut out, 1, &mut ret)`. A failed HRESULT or `ret != 1` returns `Reason::ReadFailed`; otherwise it returns `out[0] as u32`.
- [X] T013 [US1] In `spikes/cpu-temp/src/main.rs`, implement the loop: install the Ctrl+C handler; locate `AMDFamily17.bin` next to `std::env::current_exe()`; `PawnIo::open`; then every 1 s until the flag is set, call `read_smn(0x0005_9800)` → `decode_tctl` → `Outcome`, then `println!` `format_line`; return `ExitCode::SUCCESS`. Print the first line immediately (within 2 s, per the contract).

**Checkpoint**: V1 and V2 pass on the target PC. The MVP answers "can we read it at all?"

---

## Phase 4: User Story 2 - Confirm readings are accurate (P1)

**Goal**: Readings within ±2 °C of HWiNFO at idle and under load (SC-002).

**Independent Test**: quickstart V5 + V6.

### Tests for User Story 2 (write first, must fail)

- [X] T014 [P] [US2] In the `spikes/cpu-temp/src/reading.rs` tests, add tests for `classify(celsius: f32) -> Outcome`: `-0.1` gives `Unavailable(ReadFailed)`; `0.0` gives `Celsius(0.0)`; `115.0` gives `Celsius(115.0)`; `115.1` gives `Unavailable(ReadFailed)`. The rule is "A decoded value below 0.0 °C or above 115.0 °C becomes `Unavailable(ReadFailed)`" (data-model.md).

### Implementation for User Story 2

- [X] T015 [US2] Implement `pub fn classify(celsius: f32) -> Outcome` in `spikes/cpu-temp/src/reading.rs` and use it in the `spikes/cpu-temp/src/main.rs` loop between `decode_tctl` and `format_line`. T014 must pass.
- [X] T016 [US2] Create `spikes/cpu-temp/compare.ps1`, which takes `-Spike <log>` and `-Hwinfo <csv>`, joins rows on `HH:MM:SS` with the HWiNFO "CPU (Tctl/Tdie)" column, and prints the paired-sample count, the max absolute deviation and the mean absolute deviation, plus PASS if max ≤ 2.0

**Checkpoint**: V5 and V6 produce numbers for the verdict.

---

## Phase 5: User Story 3 - Clear failure instead of a crash (P2)

**Goal**: Every failure prints `HH:MM:SS  —  (<reason>)` each second, and the program keeps running.

**Independent Test**: quickstart V3 + V4.

### Tests for User Story 3 (write first, must fail)

- [X] T017 [P] [US3] In the `spikes/cpu-temp/src/reading.rs` tests, add `format_line` tests: `Unavailable(NotElevated)` gives `"14:02:07  —  (not elevated)"`, and likewise for `DriverNotFound` → `driver not found`, `UnsupportedCpu` → `unsupported CPU`, `ReadFailed` → `read failed`
- [X] T018 [P] [US3] In the same module, add tests for `is_supported(vendor: &[u8; 12], family: u32) -> bool`: `b"AuthenticAMD"` with `0x17`, `0x19` and `0x1A` gives true; `b"AuthenticAMD"` with `0x15` gives false; `b"GenuineIntel"` with `0x06` gives false

### Implementation for User Story 3

- [X] T019 [US3] Implement the `Unavailable` branch of `format_line` and `pub fn is_supported` in `spikes/cpu-temp/src/reading.rs`. T017 and T018 must pass.
- [X] T020 [US3] In `spikes/cpu-temp/src/platform.rs`, implement `pub fn is_elevated() -> bool` (`OpenProcessToken` + `GetTokenInformation(TokenElevation)`; false on any error) and `pub fn cpu_vendor_family() -> ([u8; 12], u32)` via `core::arch::x86_64::__cpuid` (leaf 0 vendor in EBX/EDX/ECX order; leaf 1 family = base + extended when base == 0xF)
- [X] T021 [US3] In `spikes/cpu-temp/src/main.rs`, run the startup checks in data-model order: `NotElevated` → `UnsupportedCpu` → `DriverNotFound` (from `PawnIo::open`). On a startup failure, print `Unavailable(reason)` every second until Ctrl+C, without retrying. On a per-tick `read_smn` error, print `Unavailable(ReadFailed)` and retry on the next tick.

**Checkpoint**: V3 and V4 pass. Running without elevation for 30 s produces no crash.

---

## Phase 6: Polish & Verdict

- [X] T022 Run the quality gates in `spikes/cpu-temp/`: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo deny check`, `cargo test`; fix any findings
- [X] T023 `cargo build --release`; confirm `target/release/cpu-temp-spike.exe` is ≤ 10 MB
- [X] T024 Run quickstart V1–V8 on the target PC and record the results
- [X] T025 Write `specs/001-cpu-temp-spike/verdict.md` (data-model "Verdict": result, evidence from V5–V8 and compare.ps1, action). If the result is Revised, add `docs/adr/0003-*.md` superseding ADR-0002 and set ADR-0002's status to superseded.

---

## Dependencies & Execution Order

- **Setup (T001–T004)** → **Foundational (T005)** → stories.
- **US1 (T006–T013)** is the MVP and must come first: US2 and US3 extend its `main.rs` loop.
- **US2 (T014–T016)** and **US3 (T017–T021)** both depend on US1. They don't depend on each other, but both edit `reading.rs` and `main.rs`, so run them one after the other.
- **Polish (T022–T025)** after all stories. T025 depends on T024.

Within each story, tests come before implementation, and `reading.rs` (pure) comes before `platform.rs` (unsafe), which comes before `main.rs` (wiring).

## Parallel Opportunities

- T002, T003, T004 together after T001.
- T006 + T007 (US1 tests); T017 + T018 (US3 tests).
- T016 (`compare.ps1`) can be written at any time after T013, alongside other work.

## Parallel Example: User Story 1

```text
Task: "T006 decode_tctl tests in spikes/cpu-temp/src/reading.rs"
Task: "T007 format_line tests in spikes/cpu-temp/src/reading.rs"   # same file: split by test fn, merge carefully
Then: T008 → T009 → T010 → T011 → T012 → T013
```

## Implementation Strategy

1. **MVP = US1**: if V1 cannot produce a plausible number, stop. That is already enough evidence to revise ADR-0002.
2. Add **US2** to measure accuracy, which is the actual decision input.
3. Add **US3** to prove graceful degradation (Constitution VIII) and give the real app its failure vocabulary.
4. Write the verdict. The spike is complete when `verdict.md` exists.

## Format Validation

All 25 tasks use `- [ ] T### [P?] [US?] description with path`. Setup, Foundational and Polish
tasks have no story label.
