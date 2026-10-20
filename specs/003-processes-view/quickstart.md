# Quickstart: Processes View

Prerequisites, build and run are as in `specs/002-tray-metrics-panel/quickstart.md`. Unit tests
additionally cover App grouping and CPU %, ordering with hover freeze, memory formatting and the
End confirmation state machine.

## Scenario validation

| # | Scenario | How | Expected |
|---|----------|-----|----------|
| P1 | Switch Views (US1) | Open the Panel, click "Processes", then "Metrics" | List appears within 300 ms with memory values; CPU values within 1.5 s; Metrics View unchanged |
| P2 | Grouping (US1) | Open a browser with several tabs | One row, `(n)` matches Task Manager's process count for it; CPU/memory are sums |
| P3 | Accuracy (SC-002) | Task Manager → Processes tab side by side, 30 s | Top 10 Apps within 5 points CPU and 10% memory |
| P4 | Live list (US1) | Start and close Notepad while watching | Row appears within 2 s, disappears within 2 s of closing |
| P5 | Remembered View/sort (FR-002) | Pick Processes + Memory sort, close and reopen the Panel | Opens on Processes, sorted by Memory |
| P6 | End with confirmation (US2) | Notepad with unsaved text → End → Confirm? | One click does nothing but arm; second click ends it; row gone within 2 s |
| P7 | Disarm (US2) | Arm a row; wait 3 s / click elsewhere / switch View / close Panel | Each returns the button to End; nothing ended |
| P8 | Re-sort safety (SC-005) | Start a CPU load in another App, arm a lower row, let the list re-sort (move the pointer out), confirm | Only the armed App is ended |
| P9 | Hover freeze (FR-010) | Keep the pointer over the list during a CPU load | Order does not change while hovering; catches up after leaving |
| P10 | Access denied (FR-016) | End `MsMpEng.exe` (Defender, protected) | Row shows `access denied` for 3 s; nothing else affected |
| P11 | System App (FR-015) | (Optional, on a test VM) End a `Service Host` | It is ended; the user accepts possible instability |
| P12 | End itself (FR-017) | End "Computer Stats" | App quits cleanly; Tray Icon removed |
| P13 | Budgets (SC-003) | `measure.ps1 -PanelOpen` with the Processes View open (200+ Processes); then Panel closed | ≤ 45 MB / < 2% CPU open; closed numbers unchanged from 002 |
| P14 | Scroll (FR-011) | Scroll to the bottom and wait 10 s | Scroll position stays put while rows update |
