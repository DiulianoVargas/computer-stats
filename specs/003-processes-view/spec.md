# Feature Specification: Processes View

**Feature Branch**: `003-processes-view`

**Created**: 2026-10-20

**Status**: Draft

**Input**: User description: "Aba secundária "Processes" no Panel do Computer Stats, ao lado da visão de Metrics atual, mostrando os processos ativos agrupados por App (como a aba Processos do Gerenciador de Tarefas: processos do mesmo executável viram uma linha com CPU % e memória somadas), com nome e ícone, ordenados por CPU (clicar no cabeçalho alterna para memória), atualizando a cada segundo. Mostra todos os processos, inclusive do sistema, sem restrição: o usuário pode encerrar qualquer App (encerra todos os processos daquele App), aceitando o risco de encerrar processos críticos do Windows já que o app roda elevado. Encerrar exige confirmação no próprio Panel: o botão vira "Confirmar?" por alguns segundos e um segundo clique encerra. Sem GPU/disco por App nesta feature."

## Context

Feature 002 delivered the Panel with every Metric. When CPU Usage or Memory Usage is high, the
user's next question is "which program is doing this?", and today the answer needs Task Manager.
This feature adds a second View to the Panel, the Processes View, that answers it and lets the
user End the offending App on the spot. Terms follow `CONTEXT.md` (App, Process, Processes View,
Metrics View, End App).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See which Apps use the PC (Priority: P1)

The user opens the Panel, switches to the Processes View, and sees every running App with its
icon, name, number of Processes, CPU use and memory use, busiest first, updated every second.

**Why this priority**: It is the reason for the View; ending an App depends on finding it first.

**Independent Test**: Start a CPU-heavy program, open the Panel, switch to the Processes View and
confirm the program is at or near the top with a CPU value close to Task Manager's.

**Acceptance Scenarios**:

1. **Given** the Panel is open on the Metrics View, **When** the user clicks the "Processes" tab,
   **Then** the Processes View replaces the Metrics View in the same Panel.
2. **Given** the Processes View is shown, **When** it appears, **Then** every running App is
   listed with icon, name, Process count (when more than one), CPU % and memory, sorted by CPU %
   from highest to lowest.
3. **Given** a browser runs 15 Processes, **When** the Processes View is shown, **Then** the
   browser is one row whose CPU % and memory are the sums of its Processes and whose count shows 15.
4. **Given** the Processes View is shown, **When** one second passes, **Then** values and order
   reflect the latest sample; Apps that started appear and Apps that exited disappear.
5. **Given** the user clicks the "Metrics" tab, **When** the Panel shows it, **Then** the Metrics
   View is exactly as in feature 002.
6. **Given** the Panel is closed with the Processes View selected, **When** the user opens the
   Panel again, **Then** it opens on the Processes View (the last View is remembered while the app
   runs).

---

### User Story 2 - End an App (Priority: P1)

The user clicks End on an App's row; the button turns into "Confirm?" for a few seconds; a second
click ends every Process of that App, and the row disappears.

**Why this priority**: It is the action the user asked for; together with US1 it is the MVP.

**Independent Test**: Open Notepad with unsaved text, End it from the Processes View with the
two-click confirmation, and confirm Notepad closed and disappeared from the list.

**Acceptance Scenarios**:

1. **Given** a row is shown, **When** the user clicks its End button once, **Then** the button
   changes to "Confirm?" and nothing is ended yet.
2. **Given** a row shows "Confirm?", **When** the user clicks it within 3 seconds, **Then** every
   Process of that App is ended and the row disappears within 2 seconds.
3. **Given** a row shows "Confirm?", **When** 3 seconds pass without a click, or the user clicks
   elsewhere, or the Panel closes, **Then** the button returns to End and nothing is ended.
4. **Given** a row shows "Confirm?" and the list re-sorts, **When** the user clicks "Confirm?",
   **Then** the App that was armed is the one ended, regardless of its new position.
5. **Given** some Processes of an App cannot be ended (e.g. protected by Windows), **When** the user
   confirms, **Then** the others are ended and the row shows a short reason (e.g. "access denied")
   for a few seconds.
6. **Given** the user Ends a Windows system App (e.g. a service host), **When** they confirm,
   **Then** it is ended like any other App; the user has accepted that this may make Windows
   unstable or restart it.

---

### User Story 3 - Sort by memory (Priority: P2)

The user clicks the Memory column header to find what is filling RAM; clicking CPU sorts by CPU
again.

**Why this priority**: Useful when Memory Usage is in Warning; CPU order covers the common case.

**Independent Test**: Click the Memory header and confirm the App with the most memory is first;
click CPU and confirm the CPU order returns.

**Acceptance Scenarios**:

1. **Given** the list is sorted by CPU, **When** the user clicks the Memory header, **Then** the
   list is sorted by memory, highest first, and the Memory header shows it is the active sort.
2. **Given** the list is sorted by memory, **When** the user clicks the CPU header, **Then** it is
   sorted by CPU again.
3. **Given** a sort was chosen, **When** the Panel is closed and reopened, **Then** the same sort
   is used.

---

### Edge Cases

- **Hundreds of Apps**: the list scrolls inside the Panel; the Panel does not grow beyond its
  maximum height.
- **The list re-sorts every second**: rows must not jump under the mouse while the user is
  hovering the list; reordering waits until the pointer leaves the list (values still update).
- **Programs without an icon or a readable name** (protected or system Processes): a generic icon
  and the program file's name are shown; a Process whose program file cannot be determined is
  grouped by its process name.
- **"System Idle Process"** is not an App and is never listed.
- **Computer Stats itself** is listed; ending it behaves like Quit (Tray Icon removed cleanly).
- **An App exits on its own while armed**: the armed state is dropped with its row; nothing else
  is ended.
- **The same program file run by several users or sessions**: all its Processes are one App.
- **Not elevated**: Apps of other users and protected system Processes may show memory as
  unavailable and cannot be ended; the attempt reports "access denied" without affecting others.
- **First second after opening the View**: CPU % needs two samples; CPU shows `—` until the first
  full second has passed, memory is shown immediately.

## Requirements *(mandatory)*

### Functional Requirements

**Views**

- **FR-001**: The Panel MUST show two tabs at its top, "Metrics" and "Processes", that switch the
  Panel between the Metrics View and the Processes View. The Metrics View is the default at
  launch.
- **FR-002**: The Panel MUST remember the last selected View and sort while the app runs and open
  on them.
- **FR-003**: Switching Views MUST NOT change the Metrics View's content or behavior from feature
  002 (Warning colors, Sparklines, Status Dot).

**Processes View content**

- **FR-004**: The Processes View MUST list every running App: every Process except the System Idle
  Process, grouped into one row per program file.
- **FR-005**: Each row MUST show the App's icon, its name (the program's description, falling back
  to the program file name), the number of Processes when more than one, CPU % and memory.
- **FR-006**: CPU % MUST be the App's share of the PC's total CPU capacity over the last second
  (all its Processes summed), shown with no decimals, matching Task Manager's Processes tab within
  5 percentage points for the same App.
- **FR-007**: Memory MUST be the App's private memory in use (all its Processes summed), shown in
  MB with no decimals below 1000 MB and in GB with one decimal from 1000 MB, matching Task
  Manager's Processes tab within 10%.
- **FR-008**: The list MUST be sorted by CPU % (highest first) by default; clicking the Memory or
  CPU header MUST sort by that column, highest first, with the active column marked. Ties are
  ordered by name.
- **FR-009**: The list MUST refresh once per second while the Processes View is shown, with Apps
  appearing and disappearing as they start and exit.
- **FR-010**: While the pointer is over the list, rows MUST NOT change order; values keep updating
  and the order catches up when the pointer leaves.
- **FR-011**: The list MUST scroll when it does not fit; the Panel keeps its maximum height.

**End App**

- **FR-012**: Each row MUST have an End button. A first click arms it: the button shows "Confirm?"
  for 3 seconds. A second click on it within that time Ends the App. Timing out, clicking anything
  else, switching View or closing the Panel disarms it. At most one row is armed at a time.
- **FR-013**: Ending an App MUST forcibly end every Process of that App; Ending MUST happen only in
  response to this two-click confirmation by the user.
- **FR-014**: The armed state MUST belong to the App, not to the row position, so a re-sort cannot
  redirect a confirmation to another App.
- **FR-015**: Any App MAY be ended, including Windows' own programs and services; no App is
  protected.
- **FR-016**: If some Processes cannot be ended, the others MUST still be ended and the row MUST
  show a short reason (e.g. "access denied") for 3 seconds; no dialog is shown.
- **FR-017**: Ending Computer Stats itself MUST behave like Quit.

**Resources**

- **FR-018**: Processes MUST be sampled only while the Processes View is visible; with the Panel
  closed or on the Metrics View, the feature MUST add no recurring work.
- **FR-019**: The app MUST stay within the constitution's resource budgets (Principle I) with the
  Processes View open.

### Key Entities

- **App (in the Processes View)**: a program file, its display name and icon, its Processes, its
  summed CPU % and summed private memory.
- **Process**: one running OS process with its program file, CPU time and private memory; belongs
  to exactly one App.
- **Sort**: CPU or Memory, highest first.
- **Armed End**: the one App currently awaiting confirmation, and when the arming expires.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Switching to the Processes View shows the list within 300 ms, with memory values
  immediately and CPU values within 1.5 seconds.
- **SC-002**: For the 10 busiest Apps, CPU % and memory agree with Task Manager's Processes tab
  within 5 percentage points and 10% respectively.
- **SC-003**: With the Processes View open, the app stays ≤ 45 MB private memory and < 2% CPU on the
  target PC with at least 200 running Processes (Constitution I); with the Panel closed, the
  closed-Panel budget is unchanged.
- **SC-004**: An App is gone from the list within 2 seconds of confirming End, in at least 95% of
  attempts on Apps that are not protected by Windows.
- **SC-005**: No App is ever ended by a single click, and no App other than the armed one is ever
  ended, across 50 trials that include re-sorting during confirmation.
- **SC-006**: A user can find and End a program that is using the most CPU within 10 seconds of
  opening the Panel.

## Assumptions

- The app runs elevated (ADR-0002), which lets it see and End nearly every Process; Processes that
  Windows protects even from administrators are reported as "access denied".
- The user explicitly accepted that Ending Windows' own programs can make the PC unstable or force
  a restart; the two-click confirmation is the only safeguard, by choice.
- Ending is forced (like Task Manager's End task); Apps are not asked to save or close gracefully.
- GPU and disk use per App, ending a single Process, suspending, restarting, or showing a Process
  tree are out of scope.
- Process names and descriptions are shown as Windows reports them; no localization is applied.
- The Panel keeps its width; the Processes View fits its columns into it.
