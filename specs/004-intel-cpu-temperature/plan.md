# Implementation Plan: CPU Temperature on Intel, and PawnIO from the installer

**Branch**: `004-intel-cpu-temperature` | **Date**: 2026-10-20 | **Spec**: [spec.md](./spec.md)

## Summary

Generalize the PawnIO source from "AMD SMN read" to a per-vendor sensor: detect the CPU kind with
CPUID, load `AMDFamily17.bin` or `IntelMSR.bin`, and decode either the AMD Tctl register or the
Intel thermal MSRs in pure, unit-tested functions. Package both modules. In the installer, a task
shown only when PawnIO is missing downloads the official `PawnIO_setup.exe` with Inno Setup's
built-in, SHA-256-verified download and runs it with `-install -silent`.

## Technical Context

**Language/Version**: Rust 1.99 (edition 2024); Inno Setup 6 Pascal script for the installer

**Primary Dependencies**: unchanged (no new crates)

**Testing**: unit tests for CPU-kind detection and Intel decoding; AMD regression on the target PC;
Intel validation on another PC (SC-001); installer run on a PC/VM without PawnIO (SC-003)

**Constraints**: Constitution I budgets unchanged; IV — the app still loads only PawnIO and signed
modules; the installer, not the app, downloads PawnIO

## Research

- **R1. Intel decoding** (Intel SDM vol. 4; LibreHardwareMonitor's IntelCpu):
  - TjMax = `IA32_TEMPERATURE_TARGET` (MSR 0x1A2) bits 23:16; 100 °C if 0.
  - Package: `IA32_PACKAGE_THERM_STATUS` (MSR 0x1B1) bits 22:16 = degrees below TjMax; available
    when CPUID.06H:EAX[6] (PTM).
  - Core fallback: `IA32_THERM_STATUS` (MSR 0x19C) bits 22:16, valid only if bit 31 is set.
  - Sensor present: CPUID.06H:EAX[0] (DTS).
- **R2. IntelMSR module**: `ioctl_read_msr` takes 1 input cell (MSR index) and returns 1 cell
  (value); it only allows a whitelist that includes 0x1A2, 0x1B1 and 0x19C (source verified).
  The module reads the MSR on the CPU the call runs on; the package MSR is the same on all cores,
  and the core fallback accepts whichever core answers.
- **R3. PawnIO setup**: `https://github.com/namazso/PawnIO.Setup/releases/download/2.2.0/PawnIO_setup.exe`,
  SHA-256 `1F519A22…3032` (winget manifest), switches `-install -silent`, success codes 0 and
  3010 (restart). Installed check: `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO`
  (the key the app already reads for the install directory).
  Inno Setup 6's `DownloadTemporaryFile(url, name, sha256, callback)` downloads and verifies.
- **R4. Pinned module hashes** (PawnIO.Modules 0.2.11): `AMDFamily17.bin` `DAE74615…7A91`,
  `IntelMSR.bin` `D6ED85D6…2E5F`.

## Constitution Check

| Principle | Status | How |
|-----------|--------|-----|
| I. Resource Budgets | ✅ | One extra MSR read at open (TjMax), one per tick, same as AMD |
| III. Unsafe Isolation | ✅ | New FFI only in `src/platform/pawnio.rs` (a generic `execute`) |
| IV. Elevated Minimal Surface | ✅ | App loads only PawnIO + signed official modules; downloading PawnIO happens in the installer, which is not the elevated app |
| VI. Dependency Discipline | ✅ | No new crates |
| VIII. Graceful Degradation | ✅ | Unsupported CPU, missing module, invalid reading and installer failures each degrade separately |
| IX. Ubiquitous Language | ✅ | No new domain terms (CPU kind is internal) |

## Project Structure

```text
src/domain/cpu_temp.rs    # CpuKind, cpu_kind(vendor, family, dts, ptm); intel decoding (+ tests)
src/platform/pawnio.rs    # CPUID leaf 6; module per kind; PawnIo::execute; Intel read
scripts/package.ps1       # ship and hash-check IntelMSR.bin too
installer/computer-stats.iss  # PawnIO task: check, download (verified), silent install
README.md                 # Intel + PawnIO install notes
```

## Complexity Tracking

No violations.
