# Data Model: Fan Speeds

## Domain (pure, `src/domain/fan.rs`, unit-tested)

### `FanChip`

| Field | Type | Notes |
|-------|------|-------|
| `name` | `&'static str` | e.g. "Nuvoton NCT6701D" (diagnostics only) |
| `channels` | `&'static [u16]` | count register per channel, in channel order (research R3) |

`fan_chip(id: u8, revision: u8) -> Option<FanChip>` — the table in research R3; `None` →
`UnsupportedMotherboard`.

### Count → speed

`fan_speed(high: u8, low: u8) -> FanSample`:

| `count = (high << 5) \| (low & 0x1F)` | Result |
|--------------------------------------|--------|
| `≥ 0x1FFF` | `Stopped` (0 RPM; also "nothing connected") |
| `0x15 ..< 0x1FFF` | `Rpm(1 350 000 / count)` |
| `< 0x15` | `Invalid` (gap) |

`Rpm` above 10 000 → `Invalid`.

### `FanList` — which channels are shown (spec FR-003)

- Holds, per channel, `seen: bool` (reported above 0 since start) and a `History`.
- `update(samples: &[FanSample])`: a channel becomes `seen` on its first `Rpm`; once seen it stays
  listed. Unseen channels are not shown and keep no history.
- Labels: `labels_for(board: &str) -> Option<&'static [&'static str]>` (research R6, empty table);
  otherwise "Fan {n}" with `n` = channel index + 1. Order = channel order.

### GPU Fan rule (research R1)

`GpuFanState { reported: bool }`: `FanRPM > 0` sets `reported`; while `!reported` the reading is
`Unavailable(NotReported)`; after, `Reading::of(MetricKind::GpuFan, rpm)`.

### Changes to existing types (`src/domain/metric.rs`)

- `MetricKind::GpuFan` — added after `GpuTemperature` in `ALL` (Panel order); `MetricKind::ALL`
  becomes 7. Validation: 0–10 000, else `ReadFailed`. Always Normal (no thresholds; spec FR-007).
- `Reason::{NotReported, UnsupportedMotherboard, LockedByFirmware}` with messages
  "not reported", "unsupported motherboard", "locked by firmware".

## Snapshot (`src/snapshot.rs`)

```text
Snapshot
  metrics: [MetricState; 7]        // + GpuFan
  fans: FansState                  // new
FansState
  Unavailable(Reason)              // one "— reason" row
  Fans(Vec<FanState>)              // may be empty: no fan has spun yet → no rows
FanState { label: String, reading: Reading, history: History, cpu_fan: bool }
```

`cpu_fan` is true only for the channel a known board labels "CPU fan" (Desktop Widget, FR-011).

## Platform (`src/platform/superio.rs`, unsafe)

### `FanSource` lifecycle

```text
open() ──► NotElevated / DriverNotFound / ModuleNotFound      (permanent, like CPU Temperature)
       ──► UnsupportedMotherboard                             (permanent)
       ──► Locked { next_check }  ──60 s──► detect again ──► Ready | Locked
       ──► Ready { base, chip }   ── read() each tick
```

- `read() -> Result<Vec<FanSample>, Reason>`: holds the ISA mutex (10 ms wait; busy → every
  channel `Invalid` this tick), reads each channel's two bytes, releases.
- A read where every channel returns `0xFF/0xFF` after having been Ready → re-detect (lock set
  again, e.g. after sleep).

### PawnIO refactor (`src/platform/pawnio.rs`)

`PawnIo::execute` generalises to `call(function, &[u64], out: &mut [u64])`; the existing one-cell
form becomes a wrapper. `PawnIo::open` is shared by `CpuTemperatureSource` and `FanSource`
(separate handles, one module each).

## Persistence

None. Fans, their `seen` state and histories live only while the app runs.
