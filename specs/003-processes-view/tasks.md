---

description: "Task list for the Processes View feature"
---

# Tasks: Processes View

**Input**: Design documents from `/specs/003-processes-view/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ui.md, quickstart.md

**Tests**: As in feature 002, every pure module in `src/domain/` gets inline `#[cfg(test)]` unit
tests in the task that creates it. Platform and UI code is validated with quickstart.md (P1–P14).

**Organization**: Grouped by user story (US1 list, US2 End App, US3 sort by memory).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1–US3
- Paths are relative to the repository root

## Conventions

Same as `specs/002-tray-metrics-panel/tasks.md`: `CONTEXT.md` terms (App, Process, Processes View,
Metrics View, End App), `unsafe` only in `src/platform/`, no `unwrap`/`expect` outside tests, every
failure degrades to a value.

---

## Phase 1: Setup

- [X] T001 Add `windows` feature `Wdk_System_SystemInformation` to `Cargo.toml` with a one-line justification ("process enumeration for the Processes View, research R1"); confirm `cargo build` and `cargo deny check` pass

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Tabs in the Panel, the sampler's control channel, and the Snapshot field every story
uses.

- [X] T002 Move the four Metric sections from `ui/panel.slint` into a new exported `MetricsView` component in `ui/metrics-view.slint` (same properties: `cpu-usage`, `cpu-temperature`, `gpu-usage`, `gpu-temperature`, `vram`, `memory-usage`, `drives`), with no visual change; `ui/panel.slint` forwards its properties to it
- [X] T003 Add tabs to `ui/panel.slint` per contracts/ui.md: `in-out property <int> view` (0 Metrics, 1 Processes), `callback view-changed(int)`, two tab buttons (active: bold + 2 px `Theme.accent-bar` underline), Ctrl+Tab toggles the View in the root `FocusScope`, the body shows `MetricsView` when `view == 0` and an empty 460 px placeholder when `view == 1`; `panel-height` follows the shown View (depends on T002)
- [X] T004 Replace the stop channel in `src/sampler.rs` with `enum Command { Stop, ProcessesVisible(bool) }` (`SamplerHandle::stop()` sends `Stop`; dropping the sender still stops); add `SamplerHandle::set_processes_visible(bool)`; on `ProcessesVisible(true)` the loop samples immediately instead of waiting for the next deadline (research R6); keep the existing `ticks_immediately_and_stops_promptly` test passing and add one for an immediate extra tick on `ProcessesVisible(true)`
- [X] T005 In `src/ui.rs`: keep the selected View in `App` (default Metrics, FR-001/FR-002), handle `view-changed` by storing it, calling `sampler.set_processes_visible(view == Processes && panel visible)`, and re-placing the Panel; also send `false` when the Panel hides and `true` when it opens on the Processes View (FR-018). Store the `SamplerHandle` in `App` so callbacks can reach it (depends on T003, T004)

**Checkpoint**: The Panel shows Metrics | Processes tabs; Processes is an empty placeholder; the
Metrics View is unchanged (FR-003); switching re-anchors the Panel.

---

## Phase 3: User Story 1 - See which Apps use the PC (Priority: P1) 🎯 MVP

**Goal**: Every running App with icon, name, Process count, CPU % and memory, sorted by CPU, live.

**Independent Test**: CPU-heavy program at or near the top, values close to Task Manager (P1–P4,
P14).

### Pure modules (with unit tests)

- [X] T006 [P] [US1] Create `src/domain/apps.rs`: `ProcessId { pid: u32, create_time: u64 }`, `ProcessSample { id, image_name, path: Option<String>, cpu_time: u64, private_bytes: u64 }`, `AppKey` (lowercase path, else `name:<lowercase image name>`), `AppUsage { key, name, path, processes: Vec<ProcessId>, cpu_pct: Option<f32>, private_bytes }`, and `group(samples, previous_cpu: &HashMap<ProcessId, u64>, elapsed_100ns: u64, logical_cpus: u32, names: impl Fn(&AppKey, &ProcessSample) -> String) -> Vec<AppUsage>` implementing data-model.md: per-Process CPU % = "Δcpu_time / (Δwall_100ns × logical_cpus) × 100, computed only when the same `ProcessId` was in the previous sample"; App CPU % = sum clamped 0–100, `None` when no Process has two samples; memory = sum; PID 0 skipped. Unit tests: grouping by path and by name fallback, PID reuse (same pid, new create_time → no CPU from the old one), clamping, `None` for all-new Apps
- [X] T007 [P] [US1] Create `src/domain/app_order.rs`: `Sort { Cpu, Memory }` (default `Cpu`) and `order(apps: &[AppUsage], sort, frozen: Option<&[AppKey]>) -> Vec<usize>`: unfrozen → by the column desc (`None` CPU sorts as 0), ties by name case-insensitive; frozen → keep the given order, drop exited keys, append new Apps at the end in sort order (FR-008, FR-010). Unit tests for both sorts, ties, freeze with appear/exit
- [X] T008 [P] [US1] Add `memory(bytes) -> String` to `src/domain/format.rs`: `"812 MB"` below 1000 MB, `"1.4 GB"` from 1000 MB (1 MB = 1024² bytes), `"0 MB"` for 0; and `app_name_with_count(name, n) -> String` adding `" (n)"` when n > 1; unit tests at the 999/1000 MB boundary

### Platform (unsafe)

- [X] T009 [US1] Create `src/platform/processes.rs`: `Enumerator` owning a reusable `Vec<u8>` buffer; `sample(&mut self) -> Option<Vec<ProcessSample>>` calling `NtQuerySystemInformation(SystemProcessInformation)`, growing the buffer on `STATUS_INFO_LENGTH_MISMATCH`, walking entries through a local `#[repr(C)]` prefix of `SYSTEM_PROCESS_INFORMATION` (`NextEntryOffset`, `NumberOfThreads`, `WorkingSetPrivateSize`, `CreateTime`, `UserTime`, `KernelTime`, `ImageName: UNICODE_STRING`, `UniqueProcessId`), each read bounds-checked against the returned length; `image_path(pid) -> Option<String>` via `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` + `QueryFullProcessImageNameW`; `logical_cpus()`; `release()` frees the buffer. Measure one sample's time with 300+ Processes and note it in `specs/003-processes-view/measurements.md` (plan risk 1) (depends on T001, T006)
- [X] T010 [P] [US1] Create `src/platform/icons.rs`: `file_description(path) -> Option<String>` (`GetFileVersionInfoSizeW` / `GetFileVersionInfoW` / `VerQueryValueW` on `\VarFileInfo\Translation`, then `\StringFileInfo\<lang><cp>\FileDescription`, trimmed, non-empty) and `small_icon(path, size_px) -> Option<IconPixels>` (`ExtractIconExW` small icon → `GetIconInfo` → `GetDIBits` 32-bpp top-down → straight RGBA, `DestroyIcon` and `DeleteObject` on every path); `IconPixels { size: u32, rgba: Vec<u8> }` defined in `src/domain/apps.rs`

### Sampler, Snapshot, UI

- [X] T011 [US1] Add `ProcessesSample { apps: Vec<AppUsage>, icons: Vec<(AppKey, IconPixels)>, own_pid: u32 }` and `processes: Option<ProcessesSample>` to `src/snapshot.rs`; in `src/sampler.rs`, while processes are visible: sample, resolve paths only for unseen `ProcessId`s (cache), resolve names/icons per new path within an ~80 ms per-tick budget (research R5; the rest fall back to the file name and arrive later), group with `domain::apps::group`, prune caches to running Processes/paths; on `ProcessesVisible(false)` clear caches and `release()` the buffer (FR-018) (depends on T004, T006, T009, T010)
- [X] T012 [P] [US1] Create `ui/app-row.slint`: `AppRowData { key: string, name: string, cpu-text: string, memory-text: string, icon: image, has-icon: bool, armed: bool, message: string, ending: bool }` and `AppRow` per contracts/ui.md columns (icon 16 px or a generic glyph, name elided, CPU 40 px, Memory 64 px, button 72 px), `callback end-clicked(string)`; button shows `End` (the armed/ending looks are filled in US2)
- [X] T013 [US1] Create `ui/processes-view.slint`: header row (Name · CPU · Memory, active sort marked `▼`, `callback sort-clicked(int)`), a `ListView` of `AppRow` bound to `in property <[AppRowData]> apps`, a `TouchArea`/`has-hover` exposing `out property <bool> list-hovered` (FR-010), and an empty-state text "Reading processes…" until the first sample; plug it into the `view == 1` slot of `ui/panel.slint` (depends on T003, T012)
- [X] T014 [US1] In `src/ui.rs`: keep one long-lived `Rc<VecModel<AppRowData>>` bound to `apps`, updated in place (`set_row_data`, `push`, `remove`) so scroll survives (FR-011, P14); on each Snapshot with `processes`, cache new icons as `slint::Image` by `AppKey` (`SharedPixelBuffer<Rgba8Pixel>`), compute the order with `app_order::order` (frozen to the previous order while `list-hovered`), and format rows with `format::memory`, `format::percent` (`—` for `None`) and `app_name_with_count`; drop the icon cache when the View is hidden (depends on T007, T008, T011, T013)
- [ ] T015 [US1] Validate P1–P4 and P14 against Task Manager and record results in `specs/003-processes-view/measurements.md`

**Checkpoint**: MVP list works; nothing can be ended yet.

---

## Phase 4: User Story 2 - End an App (Priority: P1)

**Goal**: Two-click, in-place End that ends every Process of the armed App and nothing else.

**Independent Test**: Notepad with unsaved text ended via End → Confirm? (P6–P8, P10, P12).

- [X] T016 [P] [US2] Create `src/domain/end_confirm.rs`: `EndConfirm` with `click(key, now) -> Option<AppKey>` (returns the key to End), `expire(now)`, `cancel()`, `armed() -> Option<&AppKey>`, implementing exactly the data-model state machine ("Idle ──click(k)──▶ Armed{k, until = now + 3 s}; Armed{k} ──click(k), now < until──▶ Idle + End(k); Armed{k} ──click(j ≠ k)──▶ Armed{j, now + 3 s}; Armed{k} ──now ≥ until | cancel()──▶ Idle"); unit tests for each transition, the 3 s boundary, and that no key other than the armed one is ever returned (SC-005)
- [X] T017 [US2] Add `end(processes: &[ProcessId]) -> EndOutcome { ended, denied, gone }` to `src/platform/processes.rs`: per Process `OpenProcess(PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION)` (fail → denied or gone), `GetProcessTimes` creation time must equal `create_time` else count as gone and skip (research R7: never end a reused PID), then `TerminateProcess(handle, 1)`; handles always closed
- [X] T018 [US2] Finish `ui/app-row.slint` button states per contracts/ui.md (Idle `End`; Armed `Confirm?` red background, white text; Ending disabled; `message` replaces Memory + button for 3 s, muted) and add a Panel-level `callback background-clicked()` from a `TouchArea` behind the Views in `ui/panel.slint` that cancels arming (depends on T012)
- [X] T019 [US2] Wire End in `src/ui.rs`: `end-clicked(key)` → `EndConfirm::click`; on arm start a one-shot `slint::Timer` for 3 s that calls `expire` and refreshes rows; cancel on `background-clicked`, View switch and Panel hide; on `End(key)` look up the App's `processes` in the latest Snapshot — if it contains `own_pid`, run Quit (FR-017); else call `processes::end`, mark the row `ending`, and if `denied > 0` show `access denied` for 3 s (FR-016) (depends on T014, T016, T017, T018)
- [ ] T020 [US2] Validate P6, P7, P8, P10, P12 (and P11 only on a disposable VM) and record results in `specs/003-processes-view/measurements.md`

**Checkpoint**: US1 + US2 = the requested feature.

---

## Phase 5: User Story 3 - Sort by memory (Priority: P2)

**Goal**: Clickable CPU / Memory headers, remembered while the app runs.

**Independent Test**: Memory header puts the largest App first; CPU restores (P5).

- [X] T021 [US3] In `src/ui.rs`, keep `Sort` in `App` (default CPU, FR-002); `sort-clicked(column)` sets it, updates the header's active marker and reorders immediately (unless the list is hovered, in which case on leave); the Panel reopens with the same sort (depends on T014)
- [ ] T022 [US3] Validate P5 and record the result

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T023 Run gates: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo deny check`, `cargo test`; confirm `unsafe` only under `src/platform/`, no `unwrap`/`expect` outside tests, and `TerminateProcess` is reachable only from the End flow (Constitution IV 1.2.0)
- [X] T024 Measure with `scripts/measure.ps1` (run in-process so no console steals focus): Processes View open with 200+ Processes, and Panel closed; record in `specs/003-processes-view/measurements.md`; exe size ≤ 10 MB (P13, SC-003)
- [X] T025 [P] Update `README.md` Features with the Processes View and End App (and the warning that ending Windows' own programs can make the PC unstable)

---

## Dependencies & Execution Order

- **Setup → Foundational → US1 → US2**; **US3** needs only US1's list (T014) and can run in
  parallel with US2.
- Within US1: T006–T008 and T010 in parallel; T009 after T006; T011 after T009/T010; T012 in
  parallel with platform work; T013 → T014 → T015.
- Within US2: T016 and T017 in parallel; T018 after T012; T019 last.

## Parallel Example: User Story 1

```text
Task: "T006 Create src/domain/apps.rs"
Task: "T007 Create src/domain/app_order.rs"
Task: "T008 Add memory() to src/domain/format.rs"
Task: "T010 Create src/platform/icons.rs"
Task: "T012 Create ui/app-row.slint"
```

## Implementation Strategy

1. Setup + Foundational: tabs and sampler plumbing, Metrics View unchanged.
2. US1: the list (MVP to look at).
3. US2: End App (the requested action) — US1 + US2 is the feature as asked.
4. US3: memory sort.
5. Polish: gates, budgets, README.

## Notes

- Work on branch `003-processes-view`; commit at each checkpoint.
- P11 (ending a system App) only on a disposable VM.
