# Tasks: CPU Temperature on Intel, and PawnIO from the installer

**Input**: `specs/004-intel-cpu-temperature/` (spec.md, plan.md)

**Tests**: pure decoding and detection get inline unit tests; hardware and installer behavior are
validated manually (SC-001–SC-003).

## Phase 1: User Story 1 - CPU Temperature on Intel (P1) 🎯 MVP

- [X] T001 [P] [US1] In `src/domain/cpu_temp.rs`: add `CpuKind { Amd, Intel { package: bool } }` and `cpu_kind(vendor, family, has_dts, has_ptm) -> Option<CpuKind>` (AMD for "AuthenticAMD" families 0x17/0x19/0x1A as today; Intel for "GenuineIntel" with DTS, `package` = PTM); `intel_tjmax(temperature_target) -> f32` (bits 23:16, 100 when 0); `intel_package(therm, tjmax) -> Reading` and `intel_core(therm, tjmax) -> Reading` (bits 22:16 below TjMax; core requires bit 31 valid, else `read failed`), both through `Reading::of(CpuTemperature, …)`; replace `is_supported` by `cpu_kind`. Unit tests with known register values, the TjMax=0 default, the invalid-bit case and every CPU-kind branch
- [X] T002 [US1] In `src/platform/pawnio.rs`: read CPUID leaf 6 EAX (DTS bit 0, PTM bit 6) when the max leaf ≥ 6; pick the module file by kind (`AMDFamily17.bin` / `IntelMSR.bin`); generalize `read_smn` into `execute(name, input, out_len)`; for Intel read TjMax once at open (`ioctl_read_msr` 0x1A2) and per tick 0x1B1 (package) or 0x19C (core) (depends on T001)
- [ ] T003 [US1] AMD regression on the target PC (SC-002): elevated run shows CPU Temperature as before

## Phase 2: User Story 2 - PawnIO from the installer (P2)

- [X] T004 [P] [US2] `scripts/package.ps1`: also extract `IntelMSR.bin`, check SHA-256 `D6ED85D65AB17A22F813EF98207D6D537155EE2DED5976A21CB48413C9B92E5F`, ship it in the installer and portable zip; list it in THIRD-PARTY-NOTICES
- [X] T005 [US2] `installer/computer-stats.iss`: `IntelMSR.bin` in `[Files]`; a `pawnio` task ("Install PawnIO (needed for CPU Temperature)", checked) visible only when the PawnIO uninstall key is absent; after install, download `PawnIO_setup.exe` 2.2.0 with `DownloadTemporaryFile` and SHA-256 `1F519A22E47187F70A1379A48CA604981C4FCF694F4E65B734AAA74A9FBA3032`, run it with `-install -silent`, treat 3010 as "restart needed" (message), and on any failure show a message and open https://pawnio.eu instead of failing the install; remove the old "open the PawnIO website" checkbox
- [ ] T006 [US2] Rebuild packages; on a PC or VM without PawnIO, install with defaults and confirm SC-003

## Phase 3: Polish

- [X] T007 Gates (`fmt`, `clippy -D warnings`, `deny`, `test`); README (Intel support, installer installs PawnIO); record results in `specs/004-intel-cpu-temperature/measurements.md`
- [ ] T008 Intel validation on an Intel PC against HWiNFO (SC-001)
