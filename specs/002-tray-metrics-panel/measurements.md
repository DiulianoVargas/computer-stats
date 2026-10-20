# Measurements: Tray Icon and Metrics Panel

Budgets (Constitution I): Panel closed ≤ 30 MB private / < 0.5% CPU; Panel open ≤ 45 MB / < 2% CPU;
release exe ≤ 10 MB. Measured with `scripts/measure.ps1` on the target PC (Ryzen 9 7900, Windows 11).

## Executable size

| Build | Size | Note |
|-------|------|------|
| Empty Slint window, opt-level 3 | 8.2 MiB | Slint winit backend pulls muda, arboard, webbrowser, resvg, ICU (non-optional) |
| Empty Slint window, opt-level "s" | 7.3 MiB | adopted in `Cargo.toml` |
| Empty Slint window, opt-level "z" | 6.9 MiB | kept in reserve if the budget gets tight |

## T017: Skeleton baseline (2026-10-20)

Tray Icon + hidden Panel + sampler with no sources. Run with `__COMPAT_LAYER=RunAsInvoker`
(not elevated; elevation does not change memory or CPU here), `-Minutes 2`, Panel never shown.

| Private memory (max) | CPU (mean) | Growth | Exe |
|----------------------|------------|--------|-----|
| 2.5 MB | 0.00 % | −0.1 MB | 7.38 MB |

The Slint window (winit + softbuffer) is created lazily on the first `show()`, so its cost is not in
this number; it is measured with the Panel open in US1 (T036).
