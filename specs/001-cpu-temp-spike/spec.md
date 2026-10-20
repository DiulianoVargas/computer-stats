# Feature Specification: CPU Temperature Spike

**Feature Branch**: `001-cpu-temp-spike` (no git repository yet)

**Created**: 2026-10-19

**Status**: Draft

**Input**: User description: "CPU Temperature spike: validate the riskiest assumption from ADR-0002 — that the app, running elevated as a Rust program, can read the AMD Ryzen 9 7900's CPU Temperature through the PawnIO driver without LibreHardwareMonitor or any .NET component. Deliverable is a minimal standalone test program that reads and prints CPU Temperature once per second, reports clearly when PawnIO is missing or the process is not elevated (graceful degradation, shows — instead of crashing), and whose readings can be compared against HWiNFO/LibreHardwareMonitor to confirm accuracy. Outcome decides whether ADR-0002 stands or must be revised."

## Context

This is a **spike**: a throwaway-quality experiment whose product is a decision, not a feature.
ADR-0002 commits the app to running elevated so it can read CPU Temperature through the PawnIO
driver. No ready-made library does this outside .NET (ruled out by ADR-0001 and Constitution
Principle II). If CPU Temperature cannot be read reliably, ADR-0002 must be revised before any
other work depends on it.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See live CPU Temperature (Priority: P1)

The developer launches the test program as administrator on the target PC and sees the current
CPU Temperature printed once per second until they stop it.

**Why this priority**: This is the assumption under test. If it fails, nothing else in the spike
matters.

**Independent Test**: Run the program elevated with the driver installed; a temperature line
appears every second.

**Acceptance Scenarios**:

1. **Given** the driver is installed and the program runs elevated, **When** it starts, **Then**
   it prints a CPU Temperature in °C within 2 seconds and a new reading every second after that.
2. **Given** the program is running, **When** the developer presses Ctrl+C, **Then** it exits
   cleanly with exit code 0.

---

### User Story 2 - Confirm readings are accurate (Priority: P1)

The developer runs the test program and a trusted reference monitor side by side, idle and under
load, and compares the readings.

**Why this priority**: A number that is wrong is worse than no number; the decision on ADR-0002
needs accuracy, not just a value.

**Independent Test**: Compare 60 seconds of readings with the reference monitor at idle and under
an all-core load.

**Acceptance Scenarios**:

1. **Given** the system is idle, **When** both monitors run for 60 seconds, **Then** the readings
   agree within the tolerance in SC-002.
2. **Given** an all-core CPU load is started, **When** the temperature rises, **Then** the test
   program's readings rise with the reference monitor's, within the same tolerance.

---

### User Story 3 - Clear failure instead of a crash (Priority: P2)

When the program cannot read CPU Temperature, it says why and keeps running, showing `—` in
place of the value.

**Why this priority**: Validates Constitution Principle VIII early and tells the eventual app
which failure messages it needs.

**Independent Test**: Run the program without elevation, and separately with the driver absent.

**Acceptance Scenarios**:

1. **Given** the program runs without administrator rights, **When** it starts, **Then** it prints
   `—` and a one-line reason ("not elevated") and does not crash.
2. **Given** the driver is not installed, **When** it starts, **Then** it prints `—` and a
   one-line reason ("driver not found") and does not crash.
3. **Given** the CPU is not a supported model, **When** it starts, **Then** it prints `—` and a
   one-line reason ("unsupported CPU") and does not crash.

---

### Edge Cases

- A single read fails transiently mid-run: that line shows `—`, and the next second's read is
  attempted normally.
- The reading is physically implausible (below 0 °C or above 115 °C): treat it as a failed read
  and show `—` rather than the bogus value.
- The driver is removed or the PC sleeps while the program runs: subsequent lines show `—`; no
  crash.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The program MUST print the CPU Temperature in °C, rounded to one decimal place,
  once per second.
- **FR-002**: The program MUST run as a standalone tool with no dependency on any other
  monitoring application running.
- **FR-003**: When CPU Temperature cannot be read, the program MUST print `—` with a one-line
  reason from: not elevated, driver not found, unsupported CPU, read failed.
- **FR-004**: The program MUST NOT crash or panic on any failure listed in FR-003 or in Edge
  Cases.
- **FR-005**: Each output line MUST include a timestamp, so it can be compared with the reference
  monitor's log.
- **FR-006**: The program MUST comply with the constitution's non-negotiable principles: native
  only (II), unsafe code isolated in one module (III), no network access or process spawning (IV).
- **FR-007**: The spike MUST end with a written verdict on ADR-0002: confirmed, or revised with
  the reason.

### Key Entities

- **Temperature Reading**: One sample: timestamp, value in °C or absent, and a failure reason
  when absent.
- **Verdict**: The spike's outcome: confirmed or revised, with the accuracy data supporting it.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On the target PC, run elevated with the driver installed, 100% of readings over a
  5-minute run produce a value (no `—`).
- **SC-002**: Readings stay within ±2 °C of the reference monitor's CPU temperature at idle and
  under full load.
- **SC-003**: All three failure scenarios in User Story 3 produce the correct reason and the
  program keeps running for at least 30 seconds without crashing.
- **SC-004**: The program's memory use stays under 10 MB, leaving headroom in the app's 30 MB
  budget (Constitution Principle I).
- **SC-005**: A verdict on ADR-0002 is recorded within the spike, and ADR-0002 is updated or
  superseded if the verdict is "revised".

## Assumptions

- Target hardware is the developer's PC: AMD Ryzen 9 7900 on Windows 11. Other CPUs are out of
  scope and only need to fail with "unsupported CPU".
- The PawnIO driver is installed manually by the developer for the spike; installing or
  bundling it from the app is out of scope here.
- The reference monitor is HWiNFO or LibreHardwareMonitor's "CPU Package / Tctl/Tdie" sensor.
  These are used only for comparison and are never a dependency of the program.
- "CPU Temperature" means the single package temperature (Tctl/Tdie), not per-core or per-CCD
  values. Ryzen 7000 desktop CPUs report Tctl equal to Tdie (no offset).
- Output is a console/terminal; no Panel, Tray Icon or Slint UI is part of this spike.
- The program's code may be reused in the real app's platform module, but is not required to be.
