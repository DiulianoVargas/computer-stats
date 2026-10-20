# Feature Specification: Tray Icon and Metrics Panel

**Feature Branch**: `002-tray-metrics-panel` (no git repository yet)

**Created**: 2026-10-20

**Status**: Draft

**Input**: User description: "UI de visualização do Computer Stats: Tray Icon na bandeja do Windows com Status Dot, Tray Menu (Start with Windows, Quit) e o Panel que abre ancorado acima do Tray Icon no clique esquerdo e fecha ao clicar fora. O Panel mostra todas as Metrics (CPU Usage, GPU Usage, Memory Usage, VRAM, Disk Space por Drive, CPU Temperature, GPU Temperature) com Sparklines de ~60 s e cores de Warning Level. Escopo desta feature: apenas Metrics (somente leitura); App Volumes/Audio ficam para uma feature separada, mas o layout do Panel deve reservar espaço para elas."

## Context

This is the first user-facing feature of Computer Stats. It turns the product into something the
user can live with: a Tray Icon that is always there, a Status Dot that says "something needs
attention" without opening anything, and a Panel that shows every Metric at a glance. Terms
follow `CONTEXT.md`. App Volume and Audio Level are a separate, later feature.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Glance at all Metrics (Priority: P1)

The user clicks the Tray Icon and a Panel opens just above it, showing CPU Usage, CPU
Temperature, GPU Usage, GPU Temperature, VRAM, Memory Usage and Disk Space for each Drive, all
updating every second. Clicking anywhere else closes it. The Tray Menu offers Quit.

**Why this priority**: This is the product. Without it there is nothing to see.

**Independent Test**: Launch the app, left-click the Tray Icon, watch values update for 10
seconds, click the desktop, then right-click the Tray Icon and choose Quit.

**Acceptance Scenarios**:

1. **Given** the app is running, **When** the user left-clicks the Tray Icon, **Then** the Panel
   opens anchored above the Tray Icon showing every Metric with its current value.
2. **Given** the Panel is open, **When** one second passes, **Then** every available Metric shows
   a new value.
3. **Given** the Panel is open, **When** the user clicks anywhere outside it or presses Esc,
   **Then** the Panel closes.
4. **Given** the Panel is open, **When** the user left-clicks the Tray Icon again, **Then** the
   Panel closes (toggle).
5. **Given** the app is running, **When** the user right-clicks the Tray Icon and chooses Quit,
   **Then** the Tray Icon disappears and the app exits.
6. **Given** a Metric cannot be read (e.g. no dedicated GPU, temperature driver missing), **When**
   the Panel is open, **Then** that Metric shows `—` with a short reason and every other Metric
   keeps working.

---

### User Story 2 - Know when something needs attention (Priority: P2)

Each Metric is colored by its Warning Level (Normal, Warning in amber, Critical in red), and the
Status Dot on the Tray Icon shows the worst Warning Level, so the user notices a hot CPU or full
memory without opening the Panel.

**Why this priority**: It is what makes a tray monitor useful when it is not being looked at, but
the Panel already delivers value without it.

**Independent Test**: Run a memory-heavy or CPU-heavy load until a threshold is crossed; the
Metric's color and the Status Dot change, and change back once the load stops.

**Acceptance Scenarios**:

1. **Given** every Metric is Normal, **When** the user looks at the Tray Icon, **Then** no Status
   Dot (or a neutral one) is shown.
2. **Given** Memory Usage crosses the Warning threshold, **When** the next sample arrives,
   **Then** Memory Usage turns amber in the Panel and the Status Dot turns amber.
3. **Given** one Metric is Warning and another is Critical, **When** the user looks at the Tray
   Icon, **Then** the Status Dot is red.
4. **Given** a Drive's Disk Space is at Warning, **When** no other Metric is above Normal,
   **Then** the Drive is amber in the Panel but the Status Dot stays Normal.
5. **Given** a Metric drops back below its threshold, **When** the next sample arrives, **Then**
   its color and the Status Dot return to the lower level.

---

### User Story 3 - See recent trend (Priority: P2)

Next to each Metric (except Disk Space) a Sparkline shows its last ~60 seconds, so the user can
tell a spike from a sustained load.

**Why this priority**: Adds context to the numbers; the Panel is usable without it.

**Independent Test**: Start a 20-second CPU load, open the Panel, and see the CPU Usage and CPU
Temperature Sparklines rise and fall.

**Acceptance Scenarios**:

1. **Given** the app has been running for at least 60 seconds, **When** the Panel opens, **Then**
   each Sparkline already shows the last 60 seconds of history (history is collected while the
   Panel is closed).
2. **Given** the app has just started, **When** the Panel opens, **Then** Sparklines show only the
   samples collected so far, with no fabricated values.
3. **Given** a Metric was unavailable for some seconds, **When** its Sparkline is drawn, **Then**
   those seconds appear as a gap.

---

### User Story 4 - Start with Windows (Priority: P3)

From the Tray Menu, the user turns on Start with Windows, and the app appears in the tray after
the next login, already elevated, with no UAC prompt.

**Why this priority**: Convenience; the user can launch the app by hand until this exists.

**Independent Test**: Turn the option on, sign out and back in, and confirm the Tray Icon appears
and CPU Temperature is readable; turn it off and confirm it no longer starts.

**Acceptance Scenarios**:

1. **Given** Start with Windows is off (the default), **When** the user opens the Tray Menu,
   **Then** the option is shown unchecked.
2. **Given** the user checks Start with Windows, **When** they next sign in, **Then** the app
   starts elevated without a UAC prompt and the menu item shows checked.
3. **Given** Start with Windows is on, **When** the user unchecks it, **Then** the app no longer
   starts at the next sign-in.

---

### Edge Cases

- **Not elevated**: the app still runs; CPU Temperature shows `—  (not elevated)`; everything else
  works.
- **No dedicated GPU** (or its driver is missing): GPU Usage, GPU Temperature and VRAM show `—`
  with a reason; integrated graphics are never shown as the GPU.
- **A Drive is added or removed** while the app runs (e.g. a new internal volume mounted): the
  Drive list updates within a few seconds. Removable USB drives never appear.
- **Second launch** while the app is already running: no second Tray Icon appears; the existing
  Panel opens instead.
- **Taskbar not at the bottom**, multiple monitors or display scaling above 100%: the Panel opens
  next to the Tray Icon, fully on the same screen, and is sharp at the current scaling.
- **Explorer restarts** (the taskbar is recreated): the Tray Icon reappears automatically.
- **Light and dark Windows themes**: the Panel and Tray Icon follow the current Windows theme and
  stay legible in both.
- **PC sleeps and resumes**: sampling resumes; the Sparkline shows a gap for the time asleep.
- **Many Drives** (more than fit comfortably): the Panel grows to fit up to 6 Drives; beyond that
  the Drive section scrolls.

## Requirements *(mandatory)*

### Functional Requirements

**Tray Icon and Tray Menu**

- **FR-001**: While running, the app MUST show exactly one Tray Icon and no taskbar button or
  other window.
- **FR-002**: Left-clicking the Tray Icon MUST toggle the Panel.
- **FR-003**: Right-clicking the Tray Icon MUST open the Tray Menu with exactly two items: Start
  with Windows (a checkable item) and Quit. The Tray Menu MUST NOT show Metrics.
- **FR-004**: Quit MUST remove the Tray Icon and end the app.
- **FR-005**: Start with Windows MUST be off by default; when on, the app MUST start elevated at
  the user's sign-in without a UAC prompt. The checked state MUST reflect the real system state.
- **FR-006**: Hovering the Tray Icon MUST show a tooltip with the app name and, when the Status
  Dot is not Normal, which Metric caused it.

**Panel**

- **FR-007**: The Panel MUST open anchored to the Tray Icon on the side away from the taskbar,
  fully on the Tray Icon's screen.
- **FR-008**: The Panel MUST close when the user clicks outside it, presses Esc, or left-clicks
  the Tray Icon again.
- **FR-009**: The Panel MUST show, in this order: CPU (CPU Usage, CPU Temperature), GPU (GPU
  Usage, GPU Temperature, VRAM), Memory (Memory Usage) and Drives (one row of Disk Space per
  Drive).
- **FR-010**: The Panel MUST be laid out as stacked sections, so a later App Volumes section can
  be added below the Metrics without changing the Metrics section.
- **FR-011**: Values MUST be shown as: usage in % with no decimals; temperatures in °C with no
  decimals; Memory Usage and VRAM as "used / total GB" with one decimal plus %; Disk Space as a
  bar plus "free GB of total GB" with the Drive letter and label.
- **FR-012**: Every Metric value in the Panel MUST refresh once per second while the Panel is
  open.
- **FR-013**: A Metric that cannot be read MUST show `—` and a short reason (e.g. "not
  elevated", "no GPU", "driver not found") without affecting other Metrics.

**Warning Levels and Status Dot**

- **FR-014**: Each Metric MUST have a Warning Level computed from these fixed thresholds
  (Warning at or above / Critical at or above):

  | Metric | Warning | Critical |
  |--------|---------|----------|
  | CPU Usage, GPU Usage | never | never |
  | Memory Usage | 80% | 90% |
  | VRAM | 85% | 95% |
  | Disk Space (used) | 85% | 95% |
  | CPU Temperature | 80 °C | 90 °C |
  | GPU Temperature | 80 °C | 87 °C |

- **FR-015**: To avoid flicker, a Metric MUST drop to a lower Warning Level only once it is at
  least 2 points (% or °C) below that level's threshold.
- **FR-016**: Each Metric's value MUST be colored by its Warning Level: default color for Normal,
  amber for Warning, red for Critical. An unavailable Metric has no Warning Level.
- **FR-017**: The Status Dot MUST show the worst Warning Level across all Metrics, except that
  Disk Space only counts when Critical. When the worst level is Normal, no Status Dot is shown.
- **FR-018**: Warning Levels MUST be shown by color only; the app MUST NOT show notifications,
  sounds or pop-ups.

**Sampling and Sparklines**

- **FR-019**: All Metrics MUST be sampled once per second whether or not the Panel is open, so the
  Status Dot and Sparklines stay current.
- **FR-020**: Each Metric except Disk Space MUST show a Sparkline of its last 60 samples, kept only
  in memory while the app runs.
- **FR-021**: Seconds with no reading (unavailable, or the PC asleep) MUST appear as gaps in the
  Sparkline.
- **FR-022**: The Drive list MUST include only fixed, internal volumes with a drive letter and
  MUST refresh within 5 seconds of a Drive appearing or disappearing.

**App behavior**

- **FR-023**: Launching the app while it is already running MUST NOT create a second instance; it
  MUST open the running instance's Panel.
- **FR-024**: The Tray Icon MUST reappear automatically after Explorer restarts.
- **FR-025**: The Panel and Tray Icon MUST follow the Windows light/dark theme and display
  scaling.
- **FR-026**: The app MUST comply with the constitution's non-negotiable principles, in
  particular the resource budgets (I) and graceful degradation (VIII).

### Key Entities

- **Metric**: A named observed value (e.g. CPU Temperature) with its current reading or a reason
  it is unavailable, its unit, its Warning Level, and its Sparkline history.
- **Drive**: A fixed internal volume: letter, label, total and free space, and its Disk Space
  Warning Level. Has no Sparkline.
- **Warning Level**: Normal, Warning or Critical, derived from a Metric's value and its fixed
  thresholds.
- **Sparkline history**: The last 60 one-second samples of a Metric, including gaps.
- **Status Dot**: The worst Warning Level across Metrics (Disk Space only at Critical) and the
  Metric that caused it.
- **Start with Windows setting**: On or off; stored in the system, not in an app file.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The Panel appears within 200 ms of clicking the Tray Icon, already showing current
  values (no blank or loading state).
- **SC-002**: With the Panel closed, the app uses ≤ 30 MB private memory and < 0.5% CPU; with the
  Panel open, ≤ 45 MB and < 2% CPU (Constitution I), measured over 10 minutes on the target PC.
- **SC-003**: Over a 24-hour run, memory use does not grow by more than 2 MB after the first 5
  minutes.
- **SC-004**: CPU Usage and Memory Usage agree with Task Manager within 5 percentage points, and
  GPU Usage, VRAM and GPU Temperature with the GPU vendor's own tool within 5 points or 2 °C.
- **SC-005**: Every Warning Level change is visible in the Panel and on the Status Dot within 2
  seconds of the threshold being crossed.
- **SC-006**: With the temperature driver absent, no dedicated GPU, or the app not elevated, the
  Panel still opens and every other Metric works; the app runs 10 minutes in each case without
  crashing.
- **SC-007**: A first-time user can tell which Metric is in Warning or Critical within 3 seconds
  of opening the Panel.

## Assumptions

- Target PC is the developer's: AMD Ryzen 9 7900, one NVIDIA dedicated GPU, Windows 11. Other
  hardware only needs to degrade gracefully.
- CPU Temperature reuses the reading validated in spike 001 (verdict Confirmed); its temperature
  driver is installed manually, as in the spike.
- Thresholds in FR-014 are fixed, sensible defaults for desktop hardware; they are not
  user-configurable in this feature.
- The app is launched by the user (manually or via Start with Windows); an installer is out of
  scope.
- App Volume, Audio Level and the App Volumes list are out of scope; only the Panel structure that
  will hold them (FR-010) is in scope.
- Panel text is in English, matching the glossary; localization is out of scope.
- No settings window: the only settings are in the Tray Menu.
