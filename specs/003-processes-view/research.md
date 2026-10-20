# Research: Processes View

## R1. Enumerating Processes with CPU and memory in one call

- **Decision**: `NtQuerySystemInformation(SystemProcessInformation)` (`windows` feature
  `Wdk_System_SystemInformation`) once per tick. One call returns every Process with its PID,
  image name, create time, kernel + user CPU time and **private working set**
  (`WorkingSetPrivateSize`), without opening any Process. We read entries through our own
  `#[repr(C)]` prefix of `SYSTEM_PROCESS_INFORMATION` (the `windows` crate exposes the documented,
  partly reserved layout), walking `NextEntryOffset`.
- The buffer is reused between ticks and grown on `STATUS_INFO_LENGTH_MISMATCH` (≈ 0.5–2 MB for
  200–400 Processes); it is freed when the Processes View is hidden (FR-018).
- **Rationale**: the alternative needs `OpenProcess` + `GetProcessTimes` + a memory query for each of
  ~300 Processes every second (≈ 900 syscalls and handles), which threatens the < 2% CPU budget
  and fails for protected Processes. System Informer and Task Manager use the same call.
- **Alternatives considered**: ToolHelp32 snapshot + per-Process queries (slow, see above); WMI
  (`Win32_PerfFormattedData_PerfProc_Process`: COM, slow, heavy); PDH counters (per-instance
  names collide for same-named Processes).

## R2. CPU % per App

- **Decision**: per Process, `Δ(kernel + user time) / (Δwall × logical CPUs) × 100`, keyed by
  (PID, create time) so a reused PID never inherits another Process's time. App CPU % = sum over
  its Processes, clamped to 0–100. A Process seen for the first time contributes 0 to the sum
  until its second sample; an App whose Processes are all new shows `—` (spec edge case "first
  second").
- **Rationale**: the same definition as Task Manager's Processes tab (SC-002).

## R3. Memory per App

- **Decision**: sum of `WorkingSetPrivateSize` (private working set) over the App's Processes.
- **Rationale**: Task Manager's Processes tab "Memory" column is the active private working set,
  so this matches it (FR-007, SC-002). Commit size (`PrivatePageCount`) is larger and would not
  match.

## R4. Grouping into Apps (program file)

- **Decision**: the App key is the lowercase full path of the program file, from
  `QueryFullProcessImageNameW` on an `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` handle.
  Paths are resolved only for Processes not seen before (cache keyed by PID + create time), so the
  steady state opens no handles. If the path cannot be read (some protected or pseudo Processes:
  `System`, `Registry`, `Secure System`, `Memory Compression`), the key is `name:<image name>`.
  PID 0 (System Idle Process) is skipped (FR-004).
- **Rationale**: grouping by program file is what Task Manager's grouping amounts to for the
  user's purpose, and it keeps "End App" well defined (all Processes of that file).

## R5. Display name and icon

- **Decision**: name = `FileDescription` from the program file's version resource
  (`GetFileVersionInfoSizeW` / `GetFileVersionInfoW` / `VerQueryValueW`, first translation), else the
  file name without `.exe`. Icon = the file's small icon from `ExtractIconExW` (first icon), turned
  into RGBA with `GetIconInfo` + `GetDIBits`; a generic icon (drawn in Slint) when none.
- Both are resolved **on the sampler thread**, cached per path, and sent to the UI as plain data
  (`Vec<u8>` RGBA, which is `Send`); the UI turns pixels into a `slint::Image` once per path and
  caches it. To keep SC-001 (list within 300 ms), a tick resolves new names/icons for at most
  ~80 ms; the remaining Apps show the file name and generic icon and fill in on later ticks.
- Caches drop entries for paths no longer running, and are cleared when the View is hidden.
- **Alternatives considered**: `SHGetFileInfoW` (pulls in the shell's icon cache and COM, more
  memory); bundling no icons (spec requires them).

## R6. Sampling only while visible, and waking the sampler

- **Decision**: the existing sampler thread does the work. Its control channel (today: stop only)
  becomes `enum Command { Stop, ProcessesVisible(bool) }`. On `ProcessesVisible(true)` the sampler
  samples immediately (memory values within 300 ms, SC-001) and then every tick adds an
  `Option<ProcessesSample>` to the Snapshot; on `false` it stops and frees the buffer and caches
  (FR-018). Metrics sampling is unchanged.
- **Rationale**: Constitution VII (no sensor reads on the UI thread, timer-driven); one owner of
  per-tick state; no new thread.

## R7. Ending an App safely

- **Decision**: `platform::processes::end(&[ProcessId])` on the UI thread, where
  `ProcessId = (pid, create_time)`. For each: `OpenProcess(PROCESS_TERMINATE |
  PROCESS_QUERY_LIMITED_INFORMATION)`, `GetProcessTimes` → **the creation time must equal the one
  sampled**, else skip (the PID was reused by another program: never end the wrong Process), then
  `TerminateProcess(handle, 1)`. Returns counts of ended / denied / already gone.
- Ending Computer Stats' own PID is never sent to `TerminateProcess`: the UI turns it into Quit
  (FR-017).
- Runs on the UI thread because it is a direct response to a click and each call is short
  (≈ 20 Processes × a few µs); it is not a sensor read (Constitution VII).
- **Rationale**: FR-013/FR-014 and SC-005 ("no App other than the armed one is ever ended"); the
  create-time check closes the PID-reuse hole between sampling and the click.

## R8. Confirmation state machine

- **Decision**: a pure `EndConfirm` in `domain`: `Idle | Armed { key, until }`. `click(key, now)`:
  Idle → Armed(key, now + 3 s); Armed(same key, before `until`) → returns `End(key)` and goes Idle;
  Armed(other key) → re-arms on the new key. `tick(now)` disarms after `until`; `cancel()` on
  View switch, Panel close or click elsewhere. A one-shot `slint::Timer` triggers the visual
  disarm at `until`.
- **Rationale**: FR-012/FR-014 in a unit-testable form (SC-005).

## R9. Ordering, hover freeze

- **Decision**: a pure `order(apps, sort, frozen: Option<&[AppKey]>)`: when frozen (pointer over
  the list, FR-010), Apps keep the previous order, new Apps are appended at the end and exited ones
  removed; otherwise sort by the column (desc), ties by name (case-insensitive).
- The Slint list reports hover through a `TouchArea`'s `has-hover` bound to an `out` property the
  UI reads when it applies a Snapshot.

## R10. Slint list and tabs

- **Decision**: `ListView` from `std-widgets.slint` (virtualized: only visible rows are
  instantiated, which keeps memory flat with hundreds of Apps). Rows are updated **in place** in
  one long-lived `VecModel` (`set_row_data`, `push`, `remove`) so scroll position survives updates.
  Tabs are two small custom buttons (no `TabWidget`: it would force both Views to exist and
  render). The Processes View has a fixed height of 460 logical px; the Panel re-places itself
  when switching Views (as for Drive changes in 002).
- **Columns at 320 px**: icon 16 · name (stretch, elide, "(n)" suffix) · CPU 40 · Memory 64 ·
  End button 72.

## R11. Constitution IV amendment

- **Decision**: add to Principle IV: "Apps MUST be ended only in response to direct, confirmed
  user input in the Panel." Constitution 1.1.0 → 1.2.0 (MINOR: expanded guidance).
- **Rationale**: ending Processes is a new capability of an always-elevated process; the
  constitution must bound it the same way it bounds App Volume changes. The user's choice to allow
  ending system Apps (spec FR-015) is within this rule.
