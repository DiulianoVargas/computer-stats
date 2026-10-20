# Feature Specification: CPU Temperature on Intel, and PawnIO from the installer

**Feature Branch**: `004-intel-cpu-temperature`

**Created**: 2026-10-20

**Status**: Draft

**Input**: User description: "do intel" — after asking whether CPU Temperature could work on Intel
as well as AMD, and whether the PawnIO driver could be set up for the user instead of being a
separate manual step.

## Context

CPU Temperature (feature 002) works on AMD Ryzen through the PawnIO driver and its AMD module.
On Intel CPUs it shows `— unsupported CPU`, and on any PC the user must find and install PawnIO
by hand. Windows has no native API for the CPU's real temperature (ADR-0002), so PawnIO stays the
source; this feature extends it to Intel and lets the installer set it up.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - CPU Temperature on an Intel PC (Priority: P1)

A user with an Intel CPU and PawnIO installed opens the Panel and sees CPU Temperature in °C,
with its Sparkline and Warning colors, exactly as on AMD.

**Why this priority**: It is the request; half of Windows PCs have Intel CPUs.

**Independent Test**: On an Intel PC with PawnIO, run elevated, open the Panel: CPU Temperature
shows a value within 3 °C of HWiNFO's "CPU Package" for 1 minute, rising under load.

**Acceptance Scenarios**:

1. **Given** an Intel CPU with a digital thermal sensor and PawnIO installed, **When** the app runs
   elevated, **Then** CPU Temperature shows the package temperature in °C, updated every second.
2. **Given** an AMD Ryzen CPU, **When** the app runs, **Then** CPU Temperature works as before.
3. **Given** a CPU that is neither supported AMD nor Intel with a thermal sensor, **When** the app
   runs, **Then** CPU Temperature shows `— unsupported CPU`.
4. **Given** the module for the CPU's vendor is missing, **When** the app runs, **Then** CPU
   Temperature shows `— module not found` and every other Metric works.

---

### User Story 2 - PawnIO set up by the installer (Priority: P2)

During installation, if PawnIO is not installed, the user can leave a checkbox ticked and the
installer installs the official PawnIO driver silently; CPU Temperature then works on first launch.

**Why this priority**: Removes the manual step that makes CPU Temperature fail on other PCs.

**Independent Test**: On a PC without PawnIO, run the installer with the box ticked; PawnIO
appears in Installed apps and the app shows CPU Temperature on first launch.

**Acceptance Scenarios**:

1. **Given** PawnIO is not installed, **When** the installer runs, **Then** it offers "Install
   PawnIO (needed for CPU Temperature)", ticked by default.
2. **Given** the box is ticked, **When** installation runs, **Then** the official PawnIO setup is
   downloaded from its publisher, verified, and installed silently.
3. **Given** PawnIO is already installed, **When** the installer runs, **Then** no PawnIO option
   is shown.
4. **Given** the download or its verification fails, or PawnIO's setup fails, **When** that
   happens, **Then** the installer finishes installing Computer Stats, tells the user PawnIO could
   not be installed, and offers to open the PawnIO website.
5. **Given** PawnIO asks for a restart, **When** installation ends, **Then** the user is told a
   restart is needed for CPU Temperature.

### Edge Cases

- **Hybrid Intel CPUs** (P-cores and E-cores): the package temperature covers all cores.
- **Older Intel CPUs without a package sensor**: the first core's sensor is used instead.
- **A sensor reading flagged as invalid**: that second shows a gap, as for any unavailable reading.
- **Offline PC during installation**: as acceptance scenario 2.4.
- **Uninstalling Computer Stats** leaves PawnIO installed (other apps may use it); it is removed
  from Installed apps like any program.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The app MUST read CPU Temperature on Intel CPUs that report a digital thermal sensor,
  using the package temperature when available and the first core's otherwise.
- **FR-002**: Intel temperatures MUST be computed as the CPU's maximum junction temperature (as
  reported by the CPU, 100 °C when it reports none) minus the sensor's distance to it, and MUST go
  through the same plausibility rule as AMD (outside 0–115 °C → `read failed`).
- **FR-003**: The app MUST pick the module by CPU vendor (AMD module for supported Ryzen families,
  Intel module for Intel) and MUST keep all existing degradation reasons.
- **FR-004**: Both modules MUST ship with the app (installer and portable zip), each verified
  against a pinned hash when packaging, with their license notices.
- **FR-005**: The installer MUST offer to install PawnIO only when it is not installed, ticked by
  default, and MUST install the official setup silently after verifying the downloaded file.
- **FR-006**: The installer MUST NOT bundle the PawnIO driver setup itself (its license is
  proprietary freeware); it is fetched from its publisher at install time.
- **FR-007**: A failure to get or install PawnIO MUST NOT fail the installation of Computer Stats.

### Key Entities

- **CPU kind**: AMD (supported family) or Intel (with thermal sensor); decides the module and the
  decoding.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On an Intel PC with PawnIO, CPU Temperature agrees with HWiNFO's CPU Package within
  3 °C over 1 minute idle and 1 minute under load.
- **SC-002**: On the target AMD PC, CPU Temperature is unchanged from feature 002 (within 1 °C of
  the spike's reference readings).
- **SC-003**: On a PC without PawnIO, a default installation ends with CPU Temperature working at
  first launch (after a restart only if PawnIO requests one).
- **SC-004**: Resource budgets (Constitution I) are unchanged: no extra memory or CPU beyond the
  module itself.

## Assumptions

- PawnIO.Modules 0.2.11's `IntelMSR.bin` (LGPL-2.1) exposes `ioctl_read_msr` for the thermal MSRs
  (`IA32_TEMPERATURE_TARGET`, `IA32_PACKAGE_THERM_STATUS`, `IA32_THERM_STATUS`); verified in its
  source.
- PawnIO setup 2.2.0 (winget `namazso.PawnIO`) installs silently with `-install -silent`, returns
  3010 when a restart is needed, and is published at a fixed GitHub URL with a known SHA-256.
- The developer's PC is AMD; Intel validation is done on another PC.
