# Feature Specification: Fan Speeds

**Feature Branch**: `006-fan-speeds`

**Created**: 2026-10-20

**Status**: Draft

**Input**: User description: "Fan speeds: show the speed (RPM) of the PC's fans in the Metrics
panel — CPU fan, case/system fans and GPU fan when available — each with its 60-second sparkline
like the other metrics, updated every second, reusing the existing sampler and UI. Read-only (no
fan control). Must explain unavailable fans with the same "—" reason style as temperatures. Must
respect the constitution: native only, unsafe isolated in src/platform, resource budgets, no
network. Open questions: which hardware sources (Super I/O via PawnIO modules, laptop embedded
controller, GPU via Windows/D3DKMT or vendor APIs), whether fans also appear in the Desktop
Widget, and how fans are named/labelled."

## Context

The Panel shows how hot the CPU and GPU are, but not how hard the PC is working to cool them. A
fan that has stopped, or one spinning at full speed while the PC is idle, explains noise and heat
that temperatures alone do not. Windows has no general way to read fan speeds: on the developer's
PC (desktop, ASUS motherboard, NVIDIA GPU) Windows reports no fans at all. Motherboard fans are
measured by the board's monitoring chip, which the app can reach through the PawnIO driver it
already uses for CPU Temperature. The GPU's fan is reported by the same Windows graphics
statistics the app already reads for GPU Temperature, when the GPU driver provides it.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Motherboard fan speeds in the Panel (Priority: P1)

A user opens the Panel and sees a **Fans** section listing each fan connected to the motherboard
(CPU fan and case fans) with its speed in RPM and a 60-second Sparkline, updated every second.

**Why this priority**: It is the request, and the motherboard carries most of a desktop PC's
fans, including the CPU cooler.

**Independent Test**: On a desktop PC with a supported motherboard and PawnIO installed, run the
app elevated and open the Panel: one row per connected fan shows its RPM within 5 % of HWiNFO's
reading for the same fan over 1 minute, and the value rises when the CPU is put under load.

**Acceptance Scenarios**:

1. **Given** a supported motherboard and PawnIO installed, **When** the user opens the Panel,
   **Then** the Fans section shows one row per connected fan, each with its speed in RPM and a
   Sparkline, updated every second.
2. **Given** a fan header with nothing connected, **When** the Panel shows the Fans section,
   **Then** that header has no row.
3. **Given** a fan that was spinning and stops (unplugged, or a zero-RPM mode), **When** the next
   second's values arrive, **Then** its row stays and shows `0 RPM`.
4. **Given** the motherboard's monitoring chip is not supported, **When** the user opens the Panel,
   **Then** the Fans section shows a single row `— unsupported motherboard` and every other Metric
   works.
5. **Given** PawnIO is not installed or the app is not elevated, **When** the user opens the
   Panel, **Then** the Fans section shows the same reason as CPU Temperature (`— driver not
   found`, `— not elevated`).
6. **Given** the motherboard's firmware has locked the monitoring chip against reads (as on the
   developer's ASUS board), **When** the app starts, **Then** it clears that read lock, as HWiNFO,
   iCUE and LibreHardwareMonitor do, and the fans are shown. Only if the lock cannot be cleared
   does the Fans section show `— locked by firmware`.

---

### User Story 2 - GPU fan speed (Priority: P2)

A user with a dedicated GPU sees the GPU fan's speed in RPM, with its Sparkline, in the GPU
section next to GPU Usage and GPU Temperature.

**Why this priority**: The GPU fan is often the loudest one, but it is a single row and depends on
the GPU driver reporting it.

**Independent Test**: On a PC whose GPU driver reports fan speed, open the Panel: GPU Fan shows a
value within 5 % of the GPU vendor's tool or HWiNFO over 1 minute, rising under GPU load.

**Acceptance Scenarios**:

1. **Given** a GPU whose driver reports fan speed, **When** the user opens the Panel, **Then** the
   GPU section has a Fan row with RPM and a Sparkline.
2. **Given** a GPU whose fans stop at idle (zero-RPM mode), **When** the GPU is idle, **Then** the
   row shows `0 RPM`, from the first second (as iCUE does).
3. **Given** a GPU driver that does not report fan speed, **When** the user opens the Panel,
   **Then** the GPU Fan row shows `0 RPM`: the driver reports a stopped fan and a missing reading
   alike, and a stopped fan is the common case (revised 2026-10-20, see Clarifications).
4. **Given** no dedicated GPU, **When** the user opens the Panel, **Then** the GPU Fan row shows
   `— no GPU`, as the other GPU Metrics do.

---

### User Story 3 - Fans on the Desktop Widget (Priority: P3)

A user with the Desktop Widget turned on sees the CPU fan and the GPU fan there, each with its
Sparkline. The other fans stay in the Panel, so the widget stays compact.

**Why this priority**: The widget is optional and observe-only; the two fans that follow the load
are the useful ones at a glance.

**Independent Test**: Turn on the Desktop Widget on the developer's PC: CPU Fan appears in the CPU
group and GPU Fan in the GPU group, with the same values and Sparklines as in the Panel.

**Acceptance Scenarios**:

1. **Given** the Desktop Widget is on and the board is known, **When** values update, **Then** the
   CPU group shows a Fan row with the CPU fan's speed and the GPU group a Fan row with the GPU
   fan's speed, matching the Panel.
2. **Given** the board is not known, so no fan can be identified as the CPU fan, **When** the
   widget is on, **Then** it shows no CPU Fan row (the numbered fans stay in the Panel).
3. **Given** the CPU fan or GPU fan cannot be read, **When** the widget is on, **Then** its row
   shows the same `—` reason as in the Panel.
4. **Given** no dedicated GPU, **When** the widget is on, **Then** it has no GPU Fan row.

### Edge Cases

- **Fan header with nothing connected**: never shown. A header counts as connected once it has
  reported a speed above 0 since the app started; until then it is hidden.
- **A fan that stops after spinning**: stays listed with `0 RPM` until the app restarts, so a
  stopped CPU fan is visible rather than silently disappearing.
- **Implausible readings** (above 10 000 RPM, or a sudden jump the chip reports when a reading is
  invalid): that second shows a gap in the Sparkline, as for any unavailable reading.
- **Laptops**: their fans are controlled by the laptop's embedded controller, which differs per
  manufacturer. Not supported in this feature; the Fans section shows `— unsupported motherboard`.
- **Two GPUs**: as for the other GPU Metrics, only the dedicated GPU the app already reports.
- **Pump headers** (AIO liquid coolers): shown like any fan.
- **The motherboard's own fan software** (e.g. ASUS Armoury Crate, Fan Xpert) running at the same
  time: readings keep working; the app only reads and never changes fan settings.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The Metrics View MUST show a Fans section listing each connected motherboard fan
  with its speed in RPM (whole number, e.g. `1240 RPM`) and a 60-second Sparkline, updated every
  second with the other Metrics.
- **FR-002**: On a known motherboard, fans MUST be labelled with the header names printed on the
  board (e.g. "CPU fan", "CPU optional", "Chassis fan 1", "AIO pump"), in the board's order. On
  any other board they MUST be labelled "Fan 1", "Fan 2"… by the chip's channel order. Adding a
  board needs only its header list, and a board is added only once its channel order has been
  checked on that board. The developer's board cannot be checked while its chip is locked (spike
  S1), so the list starts empty.
- **FR-003**: A fan header MUST be shown only once it has reported a speed above 0 since the app
  started, and MUST then stay listed (showing `0 RPM` when stopped) until the app restarts.
- **FR-004**: The GPU section MUST show a GPU Fan row with RPM and a Sparkline. A speed of 0 (fans
  stopped, or not reported by the driver) MUST show as `0 RPM`.
- **FR-005**: When fan speeds cannot be read, the Fans section MUST show one row with the reason,
  using the same wording as CPU Temperature where the cause is shared (`— not elevated`,
  `— driver not found`, `— module not found`, `— read failed`), plus `— unsupported
  motherboard` when the board's monitoring chip is not recognised and `— locked by firmware` when
  the firmware has locked the chip against reads.
- **FR-006**: The app MUST only read fan speeds. It MUST NOT change fan speeds, fan curves or any
  setting of the monitoring chip, with one exception: the firmware's I/O space read lock, which
  it clears when set (revised 2026-10-20, see Clarifications). The lock gates only reads of the
  sensors, affects no fan behaviour and is set again by the firmware on the next boot. The other
  writes allowed are the chip's access protocol (entering and leaving configuration mode, and
  selecting the device, bank and register to read). If the lock is still set after clearing it,
  the Fans section shows `— locked by firmware`.
- **FR-007**: Fans MUST always have the Normal Warning Level and MUST NOT affect the Status Dot.
- **FR-008**: A failure to read fans MUST NOT affect any other Metric, and a failure of the GPU
  Fan MUST NOT affect the motherboard fans or the other GPU Metrics.
- **FR-009**: Fan readings MUST coexist safely with other software that reads the same monitoring
  chip (e.g. HWiNFO, the motherboard vendor's tools): the app MUST hold the chip only for the
  duration of each read, using the system-wide lock other monitoring tools use.
- **FR-010**: Any additional PawnIO module needed MUST ship with the app (installer and portable
  zip), verified against a pinned hash when packaging, with its license notice, like the CPU
  Temperature modules.
- **FR-011**: The Desktop Widget MUST show a Fan row in its CPU group (the CPU fan, only on a known
  board) and in its GPU group (the GPU fan, when there is a dedicated GPU), with the same value,
  reason and Sparkline as the Panel. Other fans MUST NOT appear on the widget.

### Key Entities

- **Fan**: one fan header whose speed the app reads. Has a label, a current speed in RPM (or a
  gap) and a Sparkline. Belongs to the motherboard or to the GPU.
- **CPU Fan** / **GPU Fan**: the fan cooling the CPU (the board's "CPU fan" header, so only on a
  known board) and the GPU's own fan.
- **Known board**: a motherboard model whose fan header names the app ships with.
- **Fan Speed**: how fast a Fan is spinning right now, in revolutions per minute (RPM). A new
  Metric, always Normal.
- **Monitoring chip**: the motherboard component that measures fan speeds (and voltages and
  temperatures). The app recognises it to know where the fan speeds are; unrecognised chips give
  `— unsupported motherboard`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On a PC with an unlocked supported chip, every fan HWiNFO lists under the
  motherboard appears in the Panel, and each reading agrees with HWiNFO within 5 % over 1 minute
  idle and 1 minute under load, the developer's PC included.
- **SC-002**: When a GPU driver reports fan speed, GPU Fan agrees with HWiNFO within 5 % over the
  same minutes.
- **SC-003**: A fan that stops while the app runs shows `0 RPM` within 2 seconds.
- **SC-004**: On a PC where fans cannot be read, the user sees a reason within 2 seconds of opening
  the Panel, and every other Metric still updates every second.
- **SC-005**: Resource budgets (Constitution I) are unchanged: memory and CPU stay within budget
  with fans sampled every second, measured before release.

## Assumptions

- The developer's PC is a desktop with an ASUS PRIME B840-PLUS WIFI (AMD), a Ryzen 9 7900 and an
  NVIDIA GeForce GTX 1660 SUPER; Windows reports no fans through its own management interface.
- PawnIO.Modules 0.2.11 includes `LpcIO.bin`, which gives access to the motherboard's monitoring
  chip (Super I/O). The chips supported are the Nuvoton NCT6796D–NCT6799D and NCT6701D family
  (ASUS, MSI) and the ITE IT8628E, IT8686E and IT8688E (Gigabyte; research R9); others give
  `— unsupported motherboard` until added.
- Spike S1 (2026-10-20): the developer's board has a Nuvoton NCT6701D whose firmware sets the
  read lock. Since the revision of FR-006 the app clears it, so the fans are validated on the
  developer's PC.
- Spike S2 (2026-10-20): the Windows graphics statistics report the developer's GTX 1660 SUPER fan
  (830 RPM at idle); the integrated Radeon reports 0.
- The system-wide lock used by monitoring tools for the chip is the shared "ISA bus" mutex used by
  HWiNFO, LibreHardwareMonitor and vendor tools.
- No fan control, fan curves, voltages or motherboard temperatures are in scope.
- Laptop fans (embedded controller) are out of scope.

## Clarifications

### Session 2026-10-20

- Q: Should fans appear on the Desktop Widget? → A: Only the CPU fan and the GPU fan, to keep the
  widget compact; every fan is in the Panel.
- Q: How are motherboard fans named? → A: With the board's printed header names on known boards
  (the developer's ASUS PRIME B840-PLUS WIFI first), "Fan 1", "Fan 2"… otherwise. Revised after
  spike S1: the developer's board is locked, so the list starts empty (FR-002).
- Q: The developer's chip is locked by the firmware; may the app clear the lock to read the fans,
  as HWiNFO and LibreHardwareMonitor do? → A: No. The app never changes a chip setting; a locked
  chip shows `— locked by firmware` (FR-005, FR-006).
- Q (revisited 2026-10-20, after 0.4.0): should the app clear the firmware's read lock after all,
  as iCUE does? → A: Yes. The user reversed the earlier answer: the app clears only the I/O space
  lock bit, never any other chip setting (FR-006).
- Q (2026-10-20): should Gigabyte boards be supported, as on a user's B550M AORUS ELITE where iCUE
  shows every fan? → A: Yes: ITE IT8628E / IT8686E / IT8688E (research R9), validated by that
  user.
- Q (2026-10-20, after the Gigabyte test): a GPU whose fans stop at idle showed `— not reported`;
  shouldn't a stopped fan just show as not spinning? → A: Yes. GPU Fan shows `0 RPM` whenever the
  driver reports 0, like iCUE; the "not reported" state is removed (FR-004).
