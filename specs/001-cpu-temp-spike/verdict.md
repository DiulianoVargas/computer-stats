# Verdict: CPU Temperature Spike

**Status**: Final (2026-10-19)

**Result**: ✅ **Confirmed**: ADR-0002 stands.

**Target**: AMD Ryzen 9 7900, Windows 11 build 26300, PawnIO 2.2.0 (Official edition),
PawnIO.Modules 0.2.11 `AMDFamily17.bin`

## Evidence

| # | Scenario | Result | Notes |
|---|----------|--------|-------|
| V1 | Live reading | ✅ Pass | 2026-10-19 23:16: 26 consecutive readings, 49.1 → 44.5 °C, a smooth idle cool-down with no `—` |
| V2 | Clean exit | ✅ Pass | Ctrl+C gives exit code 0 |
| V3 | Not elevated | ✅ Pass | `—  (not elevated)` every second, no crash |
| V4 | Driver missing | ✅ Pass | Module file renamed gives `—  (driver not found)`, no crash |
| V5 | Accuracy, idle | ✅ Pass | 60 paired samples: **max 0.25 °C, mean 0.07 °C** |
| V6 | Accuracy, load | ✅ Pass | 89 paired samples, 24-thread busy load, 55 → 60.5 °C: **max 1.98 °C, mean 0.08 °C**. The max is a single sample at the load ramp (~2 °C/s), where the reference took two samples in the same second (55.1 and 56.9 °C). Excluding the first 3 s of the ramp, max 0.25 °C. |
| V7 | 5-min stability | ✅ Pass | 23:35:09–23:40:09 elevated: 299 readings, **0 `—`**, 49.2 → 51.4 °C |
| V8 | Memory | ✅ Pass | Elevated, sampled every 10 s for 5 min: **private 6.74–6.83 MB** (< 10 MB), working set ~5.1 MB, no growth. The unelevated run used 0.78 MB; the difference is PawnIOLib plus the loaded module. |

### V5/V6 method

The reference was LibreHardwareMonitorLib 0.9.7 ("Core (Tctl/Tdie)") instead of HWiNFO, so the run
could be scripted. Both tools ran simultaneously and were paired by `HH:MM:SS`; the reference
sampled mid-second. Script: `spikes/cpu-temp/validation/run-v5-v6.ps1`; raw logs are in the same
folder.

**Caveat**: LibreHardwareMonitor reads the same register through the same driver with the same
decoding, so this proves the spike matches the established implementation, not that the sensor
itself is calibrated. An independent cross-check against HWiNFO (which has its own driver) is
optional.

## Findings

- **Installation pitfall**: the PawnIO installer's *Unrestricted* edition is unsigned and fails
  with error 256 (`0x800B0100`, catalog has no signature) when Code Integrity is enforced. The
  *Official* edition installed and started fine with Memory Integrity (HVCI) on, on an Insider
  build. The real app's setup docs must say this.
- **Distribution pitfall**: the PawnIO.Modules GitHub release's "Source code (zip)" contains only
  `.p` sources. The signed `.bin` modules are in the `release_X_Y_Z.zip` asset.
- **Reason granularity**: a missing module file currently reports "driver not found". Consider a
  separate reason in the real app.

- **Budget headroom**: CPU Temperature costs about 7 MB private, which is a meaningful share of the
  30 MB Panel-closed budget (Constitution I). The real app should load PawnIO once and share it,
  never per Metric.

## Action

- ADR-0002 is **confirmed**; a validation note is added to it.
- `spikes/cpu-temp/src/platform.rs` (PawnIo, elevation, CPUID) and `reading.rs` (decode, classify)
  are fit to seed the real app's platform module.
- Follow-ups for the real app: document the Official-edition and `release_X_Y_Z.zip` pitfalls in
  setup docs, and decide whether "module not found" becomes its own Reason.
