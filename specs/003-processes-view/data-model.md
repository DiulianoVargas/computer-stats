# Data Model: Processes View

Pure types live in `src/domain/` (safe, I/O-free, unit-tested). Names follow `CONTEXT.md`.

## ProcessId

`(pid: u32, create_time: u64)` — identity of one Process. A PID alone is not an identity: Windows
reuses PIDs (research R7).

## ProcessSample

One Process in one tick (from the platform):

- `id: ProcessId`
- `image_name: String` (e.g. `chrome.exe`)
- `path: Option<String>` (full program file path; `None` when unreadable)
- `cpu_time: u64` (kernel + user, 100 ns units, cumulative)
- `private_bytes: u64` (private working set)

PID 0 never appears.

## AppKey

`String`: lowercase `path`, or `name:<lowercase image_name>` when the path is unknown (research R4).

## AppUsage

One row of the Processes View:

| Field | Rule |
|-------|------|
| `key: AppKey` | identity, stable across ticks |
| `name: String` | `FileDescription`, else file name without extension (R5) |
| `path: Option<String>` | for the icon |
| `processes: Vec<ProcessId>` | ≥ 1; `count` shown when > 1 |
| `cpu_pct: Option<f32>` | sum of per-Process CPU %, clamped 0–100; `None` when no Process has two samples yet (shown `—`) |
| `private_bytes: u64` | sum |

**CPU % of one Process** (R2): `Δcpu_time / (Δwall_100ns × logical_cpus) × 100`, computed only when
the same `ProcessId` was in the previous sample.

## ProcessesSample

The Snapshot's `processes: Option<ProcessesSample>` — `None` while the View is hidden.

- `apps: Vec<AppUsage>` (unordered; the UI orders)
- `icons: Vec<(AppKey, IconPixels)>` — icons resolved this tick, sent once per key
- `own_pid: u32` — Computer Stats itself (End → Quit, FR-017)

`IconPixels { size: u32, rgba: Vec<u8> }` (`size × size × 4`).

## Sort

`Cpu | Memory`, always highest first; ties by name (case-insensitive). Default `Cpu` (FR-008).
Remembered while the app runs (FR-002).

## View

`Metrics | Processes`. Default `Metrics`; remembered while the app runs (FR-001, FR-002).

## EndConfirm (state machine, FR-012/FR-014)

```text
Idle ──click(k)──▶ Armed{k, until = now + 3 s}
Armed{k} ──click(k), now < until──▶ Idle + End(k)
Armed{k} ──click(j ≠ k)──▶ Armed{j, now + 3 s}
Armed{k} ──now ≥ until | cancel()──▶ Idle
```

`cancel()` on View switch, Panel close, or a click outside any End button.

## EndOutcome

`{ ended: usize, denied: usize, gone: usize }` from `platform::processes::end`. When `denied > 0`
the row shows `access denied` for 3 s (FR-016).

## Relationships

```text
Sampler (only while Processes View visible)
  ├─ NtQuerySystemInformation ─▶ Vec<ProcessSample>
  ├─ previous cpu_time by ProcessId ─▶ cpu %
  ├─ path cache by ProcessId, name/icon cache by path
  └─▶ ProcessesSample in Snapshot ─▶ UI: order (Sort, hover freeze) ─▶ ListView rows
UI End click ─▶ EndConfirm ─▶ End(key) ─▶ platform::processes::end(processes of key)
```
