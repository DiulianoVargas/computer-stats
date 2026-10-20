# Data Model: CPU Temperature Spike

## Temperature Reading

One sample, produced once per second.

| Field     | Type                        | Notes                                     |
|-----------|-----------------------------|-------------------------------------------|
| timestamp | local time `HH:MM:SS`       | From the system clock (FR-005)            |
| outcome   | `Celsius(f32)` \| `Unavailable(Reason)` | Exactly one of the two          |

Validation rules:
- A decoded value below 0.0 °C or above 115.0 °C becomes `Unavailable(ReadFailed)` (spec Edge Cases).
- `Celsius` is printed with one decimal place (FR-001).

## Reason

Why a reading is unavailable (FR-003). Reasons are checked in this order at startup:

1. `NotElevated`: the process token is not elevated.
2. `UnsupportedCpu`: the CPU is not AMD family 17h, 19h or 1Ah.
3. `DriverNotFound`: `PawnIOLib.dll` is missing, `pawnio_open` failed, or the module file is missing
   or rejected.
4. `ReadFailed`: a single SMN read returned an error, or the value was implausible.

## Sensor state

```text
            start
              │
   ┌──────────┴──────────┐
   │ startup checks 1–3  │──fail──▶ Unavailable(reason)  (prints — every second, never retries)
   └──────────┬──────────┘
              │ ok
              ▼
           Ready ──read ok──▶ prints °C
             │  ▲
   read fail │  │ next tick
             ▼  │
     Unavailable(ReadFailed)   (transient; retried next second)
```

Startup failures are permanent for the run. Read failures are transient and are retried every
tick, which covers the driver being removed and the PC sleeping.

## Verdict

The outcome of the spike (FR-007). It is recorded in `verdict.md` in this folder.

| Field    | Notes                                                  |
|----------|--------------------------------------------------------|
| result   | `Confirmed` \| `Revised`                               |
| evidence | Idle and load comparison: max and mean deviation vs reference |
| action   | If `Revised`: the superseding ADR number               |
