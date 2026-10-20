# Measurements: Intel CPU Temperature and PawnIO from the installer

## 2026-10-20

- Unit tests: CPU kind (AMD families, Intel with/without DTS/PTM, unsupported vendors), Intel
  TjMax default, package and core decoding, invalid core reading, implausible values (96 tests
  in total pass).
- Packages: both modules extracted from PawnIO.Modules 0.2.11 and SHA-256 checked
  (`IntelMSR.bin` `D6ED85D6…2E5F`); portable zip and installer contain both.
- PawnIO setup 2.2.0: downloaded from the pinned URL, SHA-256 matches the pinned value, Authenticode
  signature valid (namazso.eu). The installer's Pascal script compiles.

## Pending

- T003: AMD regression, elevated, on the target PC.
- T006: installer with the PawnIO task on a PC/VM without PawnIO (this PC already has it, so the
  task is hidden here).
- T008: Intel PC against HWiNFO's CPU Package (SC-001).
