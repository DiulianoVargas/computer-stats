# Measurements: Fan Speeds

## 2026-10-20 — F10

Developer's PC (Ryzen 9 7900, GTX 1660 SUPER, ASUS PRIME B840-PLUS WIFI with the chip locked),
release exe elevated, Desktop Widget on, Panel closed, `scripts/measure.ps1 -Minutes 10`:

| Measure | Result | Budget |
|---------|--------|--------|
| Private memory (max) | 13.2 MB (min 10.2, mean 10.7) | ≤ 30 MB |
| CPU (mean) | 0.06 % | < 0.5 % |
| Memory growth (last − first) | 1.2 MB | — |
| Release exe | 9.04 MB | ≤ 10 MB |

All budgets met. The widget-on case is a superset of widget-off (same sampler, plus the widget's
render), so widget-off was not measured separately. On this PC the chip is locked, so the
per-tick fan read does not run (one detection per minute instead); the per-tick cost on an
unlocked chip is estimated in research R8 (< 1 ms) and is measured with F4.

## Gates

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo deny check`
(advisories, bans, licenses, sources ok) and `cargo test` (118 tests) pass.
