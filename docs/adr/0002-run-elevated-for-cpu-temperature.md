# Run as administrator to read CPU Temperature

Windows has no reliable unprivileged API for CPU temperature (the WMI thermal zone is often fake or missing), so reading it on the Ryzen CPU requires a kernel driver (PawnIO, the signed successor to the Defender-flagged WinRing0), which requires the app to run elevated. We accepted always-elevated over the alternatives of GPU-only temperatures or piggybacking on an already-running HWiNFO/LibreHardwareMonitor.

## Consequences

- Every launch is elevated (UAC prompt, or a scheduled-task trick for start-with-Windows).
- An elevated process changing other Apps' App Volume is fine, but any future feature that launches other programs will launch them elevated too.

## Validation

Confirmed by spike 001 (`specs/001-cpu-temp-spike/verdict.md`, 2026-10-19): a native Rust program
read the Ryzen 9 7900's Tctl/Tdie through PawnIO (Official edition) with HVCI on, within 0.25 °C of
LibreHardwareMonitor at idle and under load, with 0 failed reads over 5 minutes and ~7 MB private
memory.
