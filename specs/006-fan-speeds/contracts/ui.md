# UI Contract: Fan Speeds

## Metrics View

```text
CPU
  Usage                 23 %        ╱╲_╱‾╲
  Temperature          54 °C        ‾‾╲__╱
GPU
  Usage                 12 %        __╱╲__
  Temperature          48 °C        ‾‾‾‾‾‾
  Fan                830 RPM        ‾‾‾‾‾‾      ← new (MetricKind::GpuFan)
  VRAM        2.1 / 6.0 GB  35 %    ‾‾‾‾‾‾
Memory
  Usage       9.8 / 16.0 GB 61 %    ‾‾‾╲_╱
Fans                                            ← new section, after Memory, before Drives
  CPU fan            1180 RPM       ‾‾╱‾‾‾
  Chassis fan 1       742 RPM       ‾‾‾‾‾‾
  Fan 4                 0 RPM       ‾‾╲___
Drives
  …
```

- **Rows** use the existing `MetricRowData` / `MetricRow`: label, value text, no secondary text,
  Sparkline. Always the Normal colour (spec FR-007).
- **Value**: `"{rpm} RPM"`, rounded to a whole number, no thousands separator (`1240 RPM`).
- **Sparkline scale**: fixed 0–3000 RPM, clamped, like the fixed scales of the other Metrics.
- **GPU Fan row** sits between GPU Temperature and VRAM. With no dedicated GPU it shows
  `— no GPU`, like the other GPU rows.
- **Fans section**:
  - One row per shown fan, in channel order (data-model `FanList`).
  - Unavailable → one full-width line `— {reason}` in the muted colour under the header.
  - Readable but no fan has spun yet → the section is not shown.
- **Reasons**: `— not elevated`, `— driver not found`, `— module not found`, `— read failed`,
  `— unsupported motherboard`, `— locked by firmware`; GPU Fan also `— not reported`.

## Desktop Widget

```text
CPU
  Usage             23 %        ╱╲_╱‾╲
  Temperature      54 °C        ‾‾╲__╱
  Fan            1180 RPM       ‾‾╱‾‾‾      ← only on a known board (FR-011)
GPU
  Usage             12 %        __╱╲__
  Temperature      48 °C        ‾‾‾‾‾‾
  Fan             830 RPM       ‾‾‾‾‾‾      ← new
Memory
  Usage    9.8 / 16.0 GB  61 %  ‾‾‾╲_╱
```

- Same value, reason and Sparkline as the Panel. The widget's height follows its content
  (`widget-height`), so it grows by one row per fan shown.
- Other fans never appear on the widget.

## Tray

- The Status Dot and tooltip ignore fans (spec FR-007).
