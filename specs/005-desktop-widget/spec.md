# Feature Specification: Desktop Widget

**Feature Branch**: `005-desktop-widget`

**Created**: 2026-10-20

**Status**: Draft

**Input**: User description: "Desktop widget: the user can place a fixed widget on the Windows desktop. From the tray menu, "Show desktop widget" toggles a small borderless card showing the metrics (CPU usage/temp, GPU usage/temp/VRAM, memory, disks) with 60-second sparklines, updated every second, reusing the existing sampler and UI components. The widget sits at the bottom of the z-order (other windows cover it, like part of the wallpaper), is hidden from the taskbar and Alt+Tab, can be dragged anywhere, and remembers its position (and visibility) across restarts, recovering if the saved monitor is gone. Respects light/dark theme, high-DPI and multiple monitors. Must stay within the app's resource budget (~4 MB, ~0.05% CPU). Open questions to clarify: whether it should stay visible on Win+D (Show desktop) via WorkerW parenting; whether it shows everything or a compact/user-selectable subset; transparent background vs themed card."

## Context

Today every Metric is one click away in the Panel, but the Panel closes as soon as the user clicks
elsewhere. Users who want to keep an eye on temperatures or memory while they work, without
clicking anything, have no option. This feature adds the Desktop Widget: a small, always-present
card on the desktop showing Metrics, which the user turns on from the Tray Menu and places where
they like. It is observe-only: it never shows App Volumes or the Processes View. Terms follow
`CONTEXT.md` (Metric, Sparkline, Warning Level, Tray Menu, Panel); "Desktop Widget" is a new term
added to the glossary by this feature.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Show Metrics on the desktop (Priority: P1)

The user right-clicks the Tray Icon, chooses "Show desktop widget", and a small card with the
Metrics and their Sparklines appears on the desktop, updating every second. Other windows cover it
as if it were part of the wallpaper. Choosing the item again hides it.

**Why this priority**: It is the feature; everything else refines it.

**Independent Test**: Turn the Desktop Widget on from the Tray Menu, open a CPU-heavy program and
confirm the CPU Usage value and Sparkline on the desktop change within a few seconds, while the
Panel stays closed.

**Acceptance Scenarios**:

1. **Given** the Desktop Widget is off, **When** the user chooses "Show desktop widget" in the
   Tray Menu, **Then** the Desktop Widget appears on the desktop and the menu item shows as checked.
2. **Given** the Desktop Widget is shown, **When** one second passes, **Then** its values,
   Sparklines and Warning Level colors match what the Panel would show for the same sample.
3. **Given** the Desktop Widget is shown, **When** the user opens or focuses any other window over
   it, **Then** that window covers the Desktop Widget; clicking the Desktop Widget never brings it
   in front of other windows.
4. **Given** the Desktop Widget is shown, **When** the user looks at the taskbar or presses
   Alt+Tab, **Then** the Desktop Widget does not appear there.
5. **Given** the Desktop Widget is shown, **When** the user chooses the checked menu item again,
   **Then** the Desktop Widget disappears.
6. **Given** a Metric's source fails (e.g. no CPU Temperature), **When** the Desktop Widget is
   shown, **Then** that Metric shows `—` and the others keep updating, as in the Panel.

---

### User Story 2 - Place it and keep it there (Priority: P2)

The user drags the Desktop Widget to a corner of any monitor. After signing out and back in (with
Start with Windows on) or restarting the app, the Desktop Widget comes back, in the same place.

**Why this priority**: A widget that returns to a default spot or disappears after each restart
would have to be set up every day; it is what makes it "fixed".

**Independent Test**: Drag the Desktop Widget to the second monitor's top-right corner, quit and
start the app, and confirm it reappears there; then turn it off, restart, and confirm it stays off.

**Acceptance Scenarios**:

1. **Given** the Desktop Widget is shown, **When** the user drags it with the mouse, **Then** it
   follows the pointer and stays where it is dropped, on any connected monitor.
2. **Given** the user placed the Desktop Widget and quits the app, **When** the app starts again,
   **Then** the Desktop Widget is shown at the same position.
3. **Given** the Desktop Widget was off when the app quit, **When** the app starts again, **Then**
   it stays off.
4. **Given** the Desktop Widget was placed on a monitor that is no longer connected (or the screen
   resolution shrank), **When** the app starts or the display configuration changes, **Then** the
   Desktop Widget is moved fully onto a connected monitor instead of staying off-screen.
5. **Given** the user drags the Desktop Widget partly off a monitor's edge, **When** they drop it,
   **Then** it is kept fully visible on that monitor.

---

### User Story 3 - Fits any screen and theme (Priority: P3)

The Desktop Widget looks right on high-DPI and mixed-DPI monitor setups and follows Windows'
light/dark mode, like the Panel.

**Why this priority**: It is polish on top of a working widget, but a blurry or wrongly themed
card on the desktop is visible all day.

**Independent Test**: Move the Desktop Widget between a 100% and a 150% scaled monitor and switch
Windows between light and dark mode; confirm it stays sharp, correctly sized and readable.

**Acceptance Scenarios**:

1. **Given** the Desktop Widget is on a monitor scaled to 150%, **When** it is displayed, **Then**
   text and Sparklines are sharp and its size scales with the monitor's scaling.
2. **Given** the Desktop Widget is dragged to a monitor with different scaling, **When** it is
   dropped, **Then** it re-scales to that monitor.
3. **Given** Windows switches between light and dark mode, **When** the change happens, **Then**
   the Desktop Widget follows within a few seconds, without restarting the app.

---

### Edge Cases

- The user presses Win+D ("Show desktop") or clicks the far right of the taskbar: the Desktop
  Widget stays visible on the desktop, like a desktop icon (FR-014). If this PC's Windows does not
  allow that, it hides with the other windows and comes back when they are restored.
- The wallpaper is very light, very dark or busy: the Desktop Widget's text and Sparklines stay
  readable (FR-010).
- The user grabs the Desktop Widget between two lines of text, where the wallpaper shows through:
  it still drags, because its whole area responds to the mouse.
- The Panel is opened while the Desktop Widget is shown: both show the same values; the Panel
  appears above the Desktop Widget and closes as usual.
- The user drags the Desktop Widget behind the taskbar: it is kept inside the monitor's work area,
  not under the taskbar.
- Monitors are added, removed or rearranged while the app runs: the Desktop Widget keeps its
  position if it is still visible, otherwise it moves onto a connected monitor.
- The app is started for the first time with the Desktop Widget turned on: it appears in the
  top-right corner of the primary monitor's work area.
- A full-screen game or video plays: the Desktop Widget is covered and never appears above it.
- The saved position file is missing or unreadable: the Desktop Widget is off and nothing else
  in the app is affected.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The Tray Menu MUST have a checkable item "Show desktop widget" that turns the
  Desktop Widget on and off; its check mark MUST reflect whether the Desktop Widget is shown.
- **FR-002**: The Desktop Widget MUST show a fixed, compact set of Metrics: CPU Usage, CPU
  Temperature, GPU Usage, GPU Temperature and Memory Usage. Each one shows its value, Sparkline
  and Warning Level color, using the same values and rules as the Panel. VRAM and Disk Space are
  not shown, and the user cannot change the set. When the PC has no GPU, the GPU Metrics show `—`
  as in the Panel.
- **FR-003**: The Desktop Widget MUST update once per second while shown.
- **FR-004**: The Desktop Widget MUST stay behind all other windows and MUST NOT come to the front
  when clicked or dragged.
- **FR-005**: The Desktop Widget MUST NOT appear in the taskbar or in Alt+Tab, and MUST NOT take
  keyboard focus away from the window the user is working in.
- **FR-006**: The user MUST be able to move the Desktop Widget by dragging it anywhere on any
  connected monitor; it MUST be kept fully inside a monitor's work area.
- **FR-007**: The app MUST remember whether the Desktop Widget is on and its position, and restore
  both when it starts.
- **FR-008**: When its saved or current position is no longer on a connected monitor, the Desktop
  Widget MUST move fully onto a connected monitor.
- **FR-009**: The Desktop Widget MUST render sharply at each monitor's scaling and re-scale when
  moved between monitors.
- **FR-010**: The Desktop Widget MUST have a transparent background: its text and Sparklines
  are drawn directly over the wallpaper, with no card behind them. They MUST stay readable over
  light, dark and busy wallpapers. Its colors MUST follow Windows' light/dark mode, and Warning
  Level colors still apply.
- **FR-011**: The Desktop Widget MUST be observe-only: it MUST NOT show App Volumes or the
  Processes View, and MUST NOT offer End App.
- **FR-012**: Turning the Desktop Widget on or off MUST NOT change the Panel's behavior; the Panel
  MUST keep opening and closing as before.
- **FR-013**: A failure to show or position the Desktop Widget MUST NOT affect the Tray Icon,
  Status Dot or Panel.
- **FR-014**: The Desktop Widget MUST stay visible when the user uses "Show desktop" (Win+D or the
  far right of the taskbar). Where Windows does not allow that, it MUST fall back to hiding with
  the other windows and reappearing when they are restored, without errors or a lost position.
- **FR-015**: The whole rectangle of the Desktop Widget, transparent parts included, MUST respond
  to dragging.

### Key Entities

- **Desktop Widget**: The optional, transparent card on the desktop showing a compact set of
  Metrics. Attributes: on/off,
  position (monitor and point on it). Observe-only; not a View of the Panel.
- **Widget Settings**: The remembered on/off state and position, kept on this PC for the current
  Windows user and read at app start.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can turn the Desktop Widget on and place it where they want in under
  10 seconds.
- **SC-002**: After restarting the app, the Desktop Widget reappears at its saved position
  (within 1 pixel at the same scaling) in 100% of restarts with an unchanged monitor setup.
- **SC-003**: After disconnecting the monitor holding the Desktop Widget, it is fully visible on
  a connected monitor within 2 seconds, or at the next app start.
- **SC-004**: With the Desktop Widget shown, the app stays within the constitution's resource
  budgets for the Panel open state (≤ 45 MB private working set, < 2% CPU), and its idle CPU
  use with the Panel closed and the Desktop Widget shown stays below 0.5%.
- **SC-005**: Values on the Desktop Widget and in the Panel never differ for the same second.
- **SC-006**: In 20 attempts to click, drag or interact with the Desktop Widget, it never covers
  another window.
- **SC-007**: After pressing Win+D, the Desktop Widget is still visible on the desktop, on the
  Windows 10 and Windows 11 versions the app supports.
- **SC-008**: On a plain white wallpaper, a plain black wallpaper and a busy photo, in both light
  and dark mode, a tester can read every value from normal viewing distance.

## Clarifications

### Session 2026-10-20

- Q: Should the Desktop Widget stay visible on Win+D? → A: Yes, it should stay visible. Where
  Windows does not allow that, it hides with the other windows and comes back with them.
- Q: Which Metrics does it show? → A: A fixed, compact set: CPU Usage and Temperature, GPU Usage
  and Temperature, and Memory Usage. VRAM and Disk Space are left out.
- Q: Background style? → A: Transparent, with text and Sparklines drawn over the wallpaper.

## Assumptions

- The Desktop Widget is a separate surface from the Panel; the Panel's anchoring and
  close-on-click-outside behavior stay unchanged.
- It is off by default; existing users see no change until they turn it on.
- Its size is fixed (no user resizing) and determined by its content; scaling follows the
  monitor.
- It is not interactive beyond dragging: clicks do not open the Panel, and there is no right-click
  menu on the Desktop Widget itself; it is turned off from the Tray Menu.
- Settings are stored locally for the current Windows user; no other settings are added by this
  feature.
- Window placement under the taskbar is avoided by keeping the Desktop Widget in each monitor's
  work area.
- The Windows 11 Widgets board (Win+W) is out of scope; this feature is about a widget on the
  desktop itself.
- Resource budgets in the constitution apply as written; the README's "about 4 MB, 0.05% CPU"
  claim is re-measured after this feature and updated if needed.
