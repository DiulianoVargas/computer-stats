# UI Contract: Processes View

Extends `specs/002-tray-metrics-panel/contracts/ui.md`. Everything there still applies to the
Metrics View, the Tray Icon and the Tray Menu.

## Tabs

```text
┌──────────────────────────────────────┐
│  Metrics   Processes                 │   ← active tab: bold text + 2 px accent underline
│ ──────────                           │
```

- Two tabs at the top of the Panel, left-aligned, above both Views.
- Clicking a tab switches the View; the Panel re-anchors if its height changes.
- The Panel opens on the last View used (Metrics at launch).
- Keyboard: Ctrl+Tab switches View; Esc still closes the Panel.

## Processes View

Fixed height 460 logical px (Panel width stays 320 px).

```text
┌──────────────────────────────────────┐
│  Metrics   Processes                 │
│            ──────────                │
│ Name                 CPU ▼   Memory  │   ← header; ▼ marks the active sort
│ [■] Google Chrome (15)  12 %  1.4 GB [End]     │
│ [■] Visual Studio Code   4 %  812 MB [End]     │
│ [■] Service Host (78)    1 %  410 MB [Confirm?]│   ← armed: red button
│ [□] System               0 %    0 MB [End]     │
│ ...                                  │   ← scrolls
└──────────────────────────────────────┘
```

| Column | Content |
|--------|---------|
| Icon | 16 × 16 logical px; generic glyph when unknown |
| Name | App name, elided; ` (n)` appended when n > 1 Processes |
| CPU | `12 %` (integer), `—` before the second sample |
| Memory | `812 MB` below 1000 MB, `1.4 GB` from 1000 MB (1 MB = 1024² bytes) |
| Action | `End` button; `Confirm?` while armed |

- Header "CPU" and "Memory" are clickable; the active one shows `▼`.
- Values use the default text color (no Warning colors in this View).
- While the pointer is over the list, rows keep their order; values still change.
- Row feedback after a confirmed End with denied Processes: the Memory and Action columns are
  replaced by `access denied` (muted) for 3 s.

### End button states

| State | Look | Click |
|-------|------|-------|
| Idle | `End`, subtle border | arm this App (disarms any other) |
| Armed | `Confirm?`, red background, white text, for 3 s | End the App |
| Ending | disabled until the next sample | — |

Arming is cancelled by: 3 s passing, a click on another row's button (which arms that one), a
click anywhere else in the Panel, switching View, or closing the Panel.

## Ending Computer Stats

Its own row behaves like any other; confirming it runs Quit (Tray Icon removed, exit code 0).
