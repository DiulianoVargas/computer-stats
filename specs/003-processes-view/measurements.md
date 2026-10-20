# Measurements: Processes View

## T009: Enumeration cost (2026-10-20, release, not elevated)

| What | Result |
|------|--------|
| One `NtQuerySystemInformation` sample, 314 Processes | 2.5 ms, buffer 874 KB |
| Program paths for all 314 Processes (cold) | 4.9 ms; 170 readable when not elevated |
| Names + icons for 81 programs (cold) | 787 ms (≈ 10 ms each); 40 have an icon |

Names and icons are the SC-001 risk (research R5): they are resolved within an ~80 ms budget per
tick, largest-memory Apps first, so the top of the list gets them first and the rest within
≈ 10 s; Apps waiting show the file name and the generic icon.

## Live checks (2026-10-20, not elevated)

- P1/P2: the Processes View shows real icons and grouped Apps ("Discord (7)", "svchost (80)"); the
  header aligns with the columns after a padding fix.
- P3 (CPU): under a 4-job load, "Windows PowerShell" showed 17 % vs 16.8 % from `Get-Process` over
  the same interval.
- P6 (End App): **validated by the user** ending Discord with End → Confirm?. First click arms only
  ("Confirm?" in red, list frozen under the pointer), verified by screenshot.
- `platform::processes::end` is covered by a test that ends a real child process found through the
  enumeration (exit code 1), and by one that refuses a PID whose creation time differs.
- Simulated clicks were guarded (the Panel must be foreground and under the cursor); several runs
  aborted because the desktop was in use, as intended.

## T024: Budgets (release, not elevated, exe 8.48 MiB)

| Scenario | Private memory | CPU (mean) |
|----------|----------------|------------|
| Processes View open (≈ 315 Processes), 30 s, Panel closed partway by desktop focus | max 6.6 MB, mean 5.5 MB | 0.13 % |
| Panel closed afterwards, 60 s | 5.0 MB | 0.04 % |

Panel-closed CPU is unchanged from 002 (0.04 %): no recurring work while hidden (FR-018).
Panel-closed memory is 1.4 MB above 002's 3.6 MB after the View was used once: allocator memory kept
after the buffer, caches and rows are freed; it does not grow. Well within the 30 MB budget.

## Pending (user)

P4, P5 (memory sort), P7–P10, P12, P14, and an elevated run.
