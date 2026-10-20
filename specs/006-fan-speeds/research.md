# Research: Fan Speeds

Spikes run on the developer's PC on 2026-10-20: ASUS PRIME B840-PLUS WIFI, Ryzen 9 7900, GeForce
GTX 1660 SUPER + integrated Radeon, PawnIO 2.2.0, PawnIO.Modules 0.2.11. Spike code:
`examples/fan_spike.rs` (read-only; run elevated with the path to `LpcIO.bin`).

## R1. GPU Fan source

- **Decision**: `FanRPM` from `D3DKMT_ADAPTER_PERFDATA`, the struct `platform::gpu` already queries
  for GPU Temperature (ADR-0003). One more field read; no new call.
- **Spike S2 result**: the GTX 1660 SUPER reports `FanRPM = 830` at idle (HWiNFO agrees); the
  integrated Radeon reports 0.
- **Stopped vs not reported**: the driver uses 0 for both. GPU Fan shows `— not reported` until
  `FanRPM` has been above 0 once in this run, then `0 RPM` for 0 (spec US2.3). A zero-RPM GPU idle
  since boot therefore shows `— not reported` until it first spins; accepted.
- **Plausibility**: above 10 000 RPM → `read failed` for that second (gap).
- **Alternatives considered**: NVAPI / ADLX (vendor DLLs, rejected by ADR-0003); WMI `Win32_Fan`
  (reports nothing on this PC, nor on most boards).

## R2. Motherboard fan source

- **Decision**: the motherboard's Super I/O chip through PawnIO's `LpcIO.bin` module, ported from
  LibreHardwareMonitor's `LpcIO.cs` / `Nct677X.cs` (MPL-2.0; we reimplement from the register
  map, no code is copied).
- **Module interface** (`LpcIO.p`, 0.2.11): `ioctl_select_slot(slot)` (0 → port 0x2E, 1 → 0x4E),
  `ioctl_superio_inb/inw/outb(reg[, val])` on the configuration port, `ioctl_find_bars()` which
  whitelists the logical devices' I/O ranges, then `ioctl_pio_inb/outb(port[, val])` inside them.
  Every call must hold the `Global\Access_ISABUS.HTP.Method` mutex.
- **Detection** (once at open, then per R4):
  1. Hold the mutex. For slot 0, then 1: write `0x87, 0x87` to the port (Nuvoton enter), read chip
     ID `0x20` and revision `0x21`.
  2. On a supported ID (R3): `find_bars`, select logical device `0x0B` (hardware monitor), read
     the base address `0x60` twice 1 ms apart (must match, ≥ 0x100, `& 0xF007 == 0`), read the
     I/O space lock `0x28` bit 4.
  3. Write `0xAA` (exit). Release the mutex.
- **Fan read** (every tick): for each channel, two bytes of the 13-bit fan count:
  `index = base+5`, `data = base+6`; write `0x4E` (bank select) then the bank, write the register,
  read data. `count = (high << 5) | (low & 0x1F)`. `0x15 ≤ count < 0x1FFF` → `RPM = 1 350 000 /
  count`; `count ≥ 0x1FFF` → 0 RPM (stopped or nothing connected); `count < 0x15` → gap.
- **Spike S1 result**: slot 0 → chip ID `0xD8` rev `0x06` = **NCT6701D**, HWM base `0x0290`,
  **I/O space lock set** (`0x28 = 0x10`). With the lock set every HWM read returns `0xFF`
  (`count` registers all `0x1FFF`, 16-bit RPM registers all `0xFFFF`).

## R3. Supported chips

| ID | Rev | Chip | Fan channels | Count registers |
|----|-----|------|--------------|-----------------|
| 0xD4 | 0x23 | NCT6796D | 6 | 0x4B0 0x4B2 0x4B4 0x4B6 0x4B8 0x4BA |
| 0xD4 | 0x2A | NCT6796D-R | 7 | + 0x4CC |
| 0xD4 | 0x51 | NCT6797D | 7 | same |
| 0xD4 | 0x2B | NCT6798D | 7 | same |
| 0xD8 | 0x02 | NCT6799D | 7 | same |
| 0xD8 | 0x06 | NCT6701D | 7 | same |

- One register layout covers every current AMD AM4/AM5 and Intel LGA1700/1851 board using a
  Nuvoton chip. Anything else (ITE, Fintek, Nuvoton NCT6687 on MSI with its different layout) →
  `— unsupported motherboard`. Adding a chip is a table row plus validation on real hardware.
- NCT6701D firmware may leave the device disabled or put the base at `0x64`; LHM enables the
  device (a setting write). We only read: a disabled device or an invalid base →
  `— unsupported motherboard`.

## R4. The firmware lock (decision by the user)

- **Revised 2026-10-20 (after 0.4.0)**: the user asked for the app to read the fans "like iCUE",
  clearing the lock. Detection now clears bit 4 of register `0x28` of the hardware monitor device
  when it is set (read-modify-write, no other bit touched), then reads it back; still set →
  `— locked by firmware`. If every fan byte reads `0xFF` later (lock set again, e.g. after sleep),
  the next tick re-runs detection, which clears it again. The original decision follows.

- **Context**: Nuvoton NCT679x/6701D have an "I/O space lock" bit; ASUS firmware sets it. HWiNFO
  and LibreHardwareMonitor clear it before reading. It gates only I/O-port access to the
  sensors, has no effect on fan control and resets on reboot.
- **Decision (user, 2026-10-20)**: the app never clears it. A locked chip gives
  `— locked by firmware` (spec FR-005/FR-006).
- **Re-check**: another tool may clear the lock while the app runs. While locked, detection reruns
  every 60 s; once unlocked, fans appear with no restart. A clear lock is never re-set by us.
- **Consequence**: motherboard fans cannot be validated on the developer's PC. Their reading is
  covered by unit tests on the count conversion and channel rules, and validated on a PC with an
  unlocked chip before it is claimed in a release note.

## R5. Coexistence with other tools

- **Decision**: hold `Global\Access_ISABUS.HTP.Method` (create-or-open, as HWiNFO, LHM and
  vendor tools do) around each detection and each tick's read, with a 10 ms wait. If it is busy,
  that second is a gap for every motherboard fan; nothing blocks the sampler longer than 10 ms.
- **Writes**: only the access protocol (enter/exit configuration mode, device select `0x07`, bank
  select `0x4E`, index register). Every tool rewrites these on each access under the same mutex,
  so they change no state another tool relies on.
- **Rationale**: interleaving two tools' index/data accesses without the mutex yields wrong
  readings for both.

## R6. Labels and known boards

- **Decision**: a static table `board product name → [header label per channel]`, matched on the
  SMBIOS baseboard product name (`Win32_BaseBoard.Product`, read through the registry key
  `HKLM\HARDWARE\DESCRIPTION\System\BIOS\BaseBoardProduct`, so no WMI). Unknown board → "Fan N".
- **Starts empty**: the developer's board is locked, so its channel order cannot be checked
  (spec FR-002). The CPU Fan row of the Desktop Widget therefore appears only on boards added later.

## R7. Shipping the module

- **Decision**: ship `LpcIO.bin` next to the exe like `AMDFamily17.bin` and `IntelMSR.bin`, pinned
  in `scripts/package.ps1`: SHA-256
  `B3896A1CAB0D808FCA31FE2EBCAE045D59DAC690DA87B17C858BB8DA357EB45E` (PawnIO.Modules 0.2.11,
  LGPL-2.1, notice already included).
- The module is loaded into its own PawnIO handle (one module per handle), opened on the sampler
  thread only when a supported chip is found.

## R8. Cost

- Per tick: up to 7 channels × 2 bytes × 4 port operations = 56 PawnIO calls (≈ 5 µs each), inside
  one mutex hold: well under 1 ms. While locked or unsupported: nothing per tick, one detection
  every 60 s (locked) or never (unsupported).
- Memory: one `History` (60 × `Option<f32>`) per shown fan, at most 8 (7 + GPU).

## R9. ITE chips (Gigabyte boards)

- **Context (2026-10-20)**: a user's PC with a Gigabyte B550M AORUS ELITE showed every fan in
  iCUE but would get `— unsupported motherboard` here: Gigabyte boards use ITE chips.
- **Decision**: support the ITE IT8628E, IT8686E and IT8688E, which share one register layout and
  need no bank select (LibreHardwareMonitor `IT87XX.cs`, reimplemented from the register map).
  IT8689E (banked registers) and the second ITE chip some Gigabyte boards have (IT879x on port
  0x4E, behind a Gigabyte embedded controller) are left out.
- **Detection**: after the Nuvoton attempt on port 0x2E fails, enter ITE configuration mode with
  `87 01 55 55`, read the 16-bit chip ID (`0x20` high, `0x21` low), `find_bars`, select logical
  device `0x04` (environment controller), read the base `0x60` twice; exit by writing `0x02` to
  configuration register `0x02`. ITE chips have no read lock.
- **Fan read**: no banks; index `base+5`, data `base+6`. Channel registers (low / high):
  `0x0D/0x18`, `0x0E/0x19`, `0x0F/0x1A`, `0x80/0x81`, `0x82/0x83`, `0x4C/0x4D`.
  `count = high << 8 | low`: `0xFFFF` → stopped; `≤ 0x3F` → misread; else
  `RPM = 1 350 000 / (2 × count)`.
- **Channels 4–6** count only when enabled in register `0x0C` (bit 4, bit 5, bit 2); disabled
  channels are never shown. Read once at detection.
- **Validation**: unit tests on the conversion and the mode bits; on hardware by the Gigabyte
  B550M's user (quickstart G1–G2). Not checked on the developer's PC (Nuvoton).
