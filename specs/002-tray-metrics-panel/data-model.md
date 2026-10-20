# Data Model: Tray Icon and Metrics Panel

All types live in pure (safe, I/O-free) modules unless noted. Names follow `CONTEXT.md`.

## MetricKind

The six per-PC Metrics shown with a Sparkline.

`CpuUsage | CpuTemperature | GpuUsage | GpuTemperature | Vram | MemoryUsage`

Disk Space is per Drive and modeled separately (`Drive`).

| Kind | Unit | Sparkline scale |
|------|------|-----------------|
| CpuUsage, GpuUsage | % | 0–100 |
| MemoryUsage, Vram | % of total (with used/total bytes) | 0–100 |
| CpuTemperature, GpuTemperature | °C | 20–100 |

## Reading

One sample of one Metric.

- `Value { value: f32, used_bytes: Option<u64>, total_bytes: Option<u64> }` (bytes only for
  MemoryUsage and Vram), or
- `Unavailable(Reason)`

Validation: percentages are clamped to 0–100; temperatures outside 0–115 °C become
`Unavailable(ReadFailed)` (rule from spike 001).

## Reason

Why a Metric or Drive is unavailable. Message text is shown in the Panel after `—`.

| Variant | Message |
|---------|---------|
| NotElevated | not elevated |
| UnsupportedCpu | unsupported CPU |
| DriverNotFound | driver not found |
| ModuleNotFound | module not found |
| NoGpu | no GPU |
| ReadFailed | read failed |

## WarningLevel

`Normal < Warning < Critical` (ordered).

**Thresholds** (Warning ≥ / Critical ≥), from spec FR-014:

| Metric | Warning | Critical |
|--------|---------|----------|
| CpuUsage, GpuUsage | — | — |
| MemoryUsage | 80 | 90 |
| Vram | 85 | 95 |
| Disk Space (% used) | 85 | 95 |
| CpuTemperature | 80 | 90 |
| GpuTemperature | 80 | 87 |

**State transition (hysteresis, FR-015)**, given previous level `p` and value `v`:

1. `raw` = the highest level whose threshold `v` meets.
2. If `raw ≥ p` → new level is `raw` (rising is immediate).
3. Else, step down one level at a time while `v < threshold(current) − 2`; the result is the
   highest level for which that check fails.

Example (Memory): 81 → Warning; 79 → Warning; 77.9 → Normal; 91 → Critical; 89 → Critical;
87.9 → Warning.

An `Unavailable` reading resets the level to `None` (no Warning Level, FR-016); the next value
starts from Normal.

## History (Sparkline history)

Fixed-size ring buffer of 60 `Option<f32>` (None = gap), one per Metric. Capacity never changes
(Constitution VII).

- `push(Some(v))` per tick with a value; `push(None)` for unavailable seconds and sleep gaps.
- `iter()` yields oldest → newest, with only the samples pushed so far (no fabricated values
  before the first tick).

## Drive

- `letter: char`, `label: String` (may be empty), `total_bytes: u64`, `free_bytes: u64`
- Derived: `used_pct = (total − free) / total × 100`
- `level: Option<WarningLevel>` (hysteresis as above)
- or unavailable with a `Reason`
- Identity: drive letter. Sorted by letter. Only fixed, non-USB volumes.

## StatusDot

- `level: WarningLevel` = max over: every Metric's level, plus each Drive's level **only if
  Critical** (FR-017).
- `cause: Option<String>`: the display name of the Metric (or Drive, e.g. `Disk C:`) with the worst
  level; ties go to the first in Panel order.
- Rendered on the Tray Icon only when `level > Normal`.

## Snapshot

Immutable result of one sampler tick, sent to the UI thread.

- `metrics: [MetricState; 6]`, each with `kind`, `reading`, `level`, `history` (copy of 60 samples)
- `drives: Vec<DriveState>`
- `status_dot: StatusDot`
- `tooltip: String`: `"Computer Stats"`, or `"Computer Stats — <cause> <level>"` when not Normal

## StartWithWindows

`On | Off`, read from Task Scheduler at startup (`schtasks /Query`), cached in the UI thread, and
updated after each successful change. Not persisted by the app.

## Relationships

```text
Sampler ──tick──▶ Snapshot ──▶ UI thread ─┬─▶ Tray Icon (StatusDot, tooltip)
   │                                       └─▶ Panel (only while visible)
   ├─ owns History ×6, WarningLevel state ×6 + per Drive
   └─ owns sources: CpuTimes, MemoryStatus, Drives, PawnIo, Nvml
```
