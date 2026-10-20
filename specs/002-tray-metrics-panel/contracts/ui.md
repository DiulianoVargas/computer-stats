# UI Contract: Tray Icon, Tray Menu and Panel

What the user sees and can do. Tests and the quickstart check against this.

## Tray Icon

- One icon, in the notification area, for the app's whole lifetime (re-added after Explorer
  restarts).
- Glyph: three ascending bars, white on a dark taskbar, near-black on a light one.
- Status Dot: lower-right filled circle; amber for Warning, red for Critical; absent when Normal.
- Tooltip: `Computer Stats`, or `Computer Stats — <cause> <Warning|Critical>`, e.g.
  `Computer Stats — CPU Temperature Critical`, `Computer Stats — Disk C: Critical`.

| Input | Result |
|-------|--------|
| Left click / Enter or Space with keyboard focus | Toggle the Panel |
| Right click / Shift+F10 / Menu key | Open the Tray Menu |

## Tray Menu

```text
┌───────────────────────────┐
│ ✓ Start with Windows      │
│ ───────────────────────── │
│   Quit                    │
└───────────────────────────┘
```

- `Start with Windows`: checkmark shows the real state. Clicking toggles it. If the change fails,
  the checkmark keeps the old state (no dialog).
- `Quit`: removes the Tray Icon and exits with code 0.

## Panel

Width 320 logical px; height fits content (max 640, Drives scroll after 6). Rounded corners,
1 px border, theme background, 12 px padding, 8 px between sections, 2 px between rows
(compacted on 2026-10-20 after user feedback: "muito espaço em branco").

```text
┌────────────────────────────────────────┐
│ CPU                                    │
│ Usage          23 %      ╱╲__╱‾‾╲_     │
│ Temperature    54 °C     ___╱‾‾‾‾      │
│                                        │
│ GPU                                    │
│ Usage           7 %      _____╱╲__     │
│ Temperature    41 °C     ‾‾‾‾‾‾‾‾‾     │
│ VRAM      2.1 / 12.0 GB  18 %  ‾‾‾‾‾   │
│                                        │
│ Memory                                 │
│ Usage    18.4 / 31.9 GB  58 %  ‾‾‾‾‾   │
│                                        │
│ Drives                                 │
│ C: System   ████████░░  212 GB free of 931 GB │
│ D: Data     ███░░░░░░░  1.4 TB free of 1.8 TB │
│                                        │
│ (App Volumes section — added by a later feature) │
└────────────────────────────────────────┘
```

### Rows

| Row | Value text | Sparkline |
|-----|-----------|-----------|
| CPU · Usage | `23 %` | yes, 0–100 |
| CPU · Temperature | `54 °C` | yes, 20–100 °C |
| GPU · Usage | `7 %` | yes |
| GPU · Temperature | `41 °C` | yes |
| GPU · VRAM | `2.1 / 12.0 GB` + `18 %` | yes |
| Memory · Usage | `18.4 / 31.9 GB` + `58 %` | yes |
| Drives · one per Drive | `C: <label>` + bar + `212 GB free of 931 GB` | no |

- Numbers: percentages and temperatures rounded to integers; GB with one decimal; sizes ≥ 1000 GB
  shown as TB with one decimal. 1 GB = 1024³ bytes (matches Explorer and Task Manager).
- Drive with an empty label: `C:` only.
- Unavailable: value text is `—` followed by the reason in a muted color, e.g. `— no GPU`,
  `— not elevated`; Sparkline keeps its gaps; no Warning color.

### Colors

| Warning Level | Value text and Drive bar |
|---------------|--------------------------|
| Normal | default text / accent bar |
| Warning | amber |
| Critical | red |

The Sparkline stroke uses the same color as the value.

### Keyboard and focus

- The Panel takes focus when shown. Esc closes it.
- The Panel is read-only in this feature: no focusable controls besides the root.

### Closing

Click outside, Esc, or left-click on the Tray Icon. Closing never quits the app.

## Start with Windows task

| Property | Value |
|----------|-------|
| Task name | `Computer Stats` (root folder) |
| Trigger | At log on of the current user |
| Run level | Highest available |
| Action | Current executable path, no arguments |
| Power | Starts and keeps running on battery |
| Time limit | None |
| Multiple instances | Ignore new |
