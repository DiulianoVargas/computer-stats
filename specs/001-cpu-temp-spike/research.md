# Research: CPU Temperature Spike

## R1. How PawnIO is called from user mode

- **Decision**: Load `PawnIOLib.dll` at runtime and call its four C exports, all returning `HRESULT`:
  - `pawnio_open(HANDLE* out)`
  - `pawnio_load(HANDLE h, const u8* blob, usize size)`
  - `pawnio_execute(HANDLE h, const char* fn, const u64* in, usize in_len, u64* out, usize out_len, usize* ret_len)`
  - `pawnio_close(HANDLE h)`
- **DLL location**: the `InstallLocation` value under
  `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\PawnIO`, falling back to
  `C:\Program Files\PawnIO\PawnIOLib.dll`.
- **Rationale**: Runtime loading means a missing driver is a recoverable error ("driver not found")
  instead of a failure to start (FR-003, FR-004).
- **Alternatives considered**: Talking to the device through `DeviceIoControl` directly is
  undocumented and could break between PawnIO versions. Linking at build time would fail to start
  when the DLL is missing.

## R2. Which PawnIO module, and which function

- **Decision**: Use the signed `AMDFamily17.bin` module from the PawnIO.Modules releases and call
  `ioctl_read_smn` with `in[0] = 0x00059800`, which returns `out[0]` as the raw register value.
- **Rationale**: PawnIO only runs signed modules. `AMDFamily17` covers Zen–Zen 5 (family 17h/19h/1Ah)
  and exposes the SMN read, which is how LibreHardwareMonitor reads Ryzen temperatures.
- **Distribution for the spike**: `AMDFamily17.bin` is placed next to the executable (see
  quickstart). Bundling it with the app is out of scope.

## R3. Converting the raw value to °C (Zen 4, family 19h model 61h)

- **Decision**: These rules match LibreHardwareMonitor's `Amd17Cpu.cs`:
  - `raw = read_smn(0x00059800)` (THM_TCON_CUR_TMP)
  - `t = (raw >> 21) * 0.125` °C
  - If `raw & 0x80000 != 0` (range select) **or** `raw & 0x30000 == 0x30000` (TJ select), apply
    `t -= 49.0`.
  - On Ryzen 7000 desktop CPUs, Tctl equals Tdie, so no further offset applies.
- **Rationale**: This is the established, field-tested decoding, and a pure function makes it
  unit-testable without hardware.

## R4. Checking that the CPU is supported

- **Decision**: Read `CPUID` leaf 0 for the vendor (`AuthenticAMD`) and leaf 1 for the family. A
  CPU is supported when it is AMD with a family of 17h, 19h or 1Ah.
- **Rationale**: This lets the program report "unsupported CPU" up front, rather than decoding
  garbage from an SMN read on Intel hardware.

## R5. Checking elevation

- **Decision**: Read the process token's `TokenElevation` through `OpenProcessToken` and
  `GetTokenInformation`.
- **Rationale**: This gives the "not elevated" message directly, instead of relying on PawnIO's
  `E_ACCESSDENIED`. That error is still mapped to "not elevated" as a fallback.

## R6. Crates

- **Decision**: Use only the `windows` crate, with default features off and only these enabled:
  `Win32_Foundation`, `Win32_System_LibraryLoader`, `Win32_System_Registry`,
  `Win32_Security`, `Win32_System_Threading`, `Win32_System_Console`,
  `Win32_System_SystemInformation`.
- **Rationale**: Constitution Principle VI. The `windows` crate is needed by the real app anyway.
  Timestamps come from `GetLocalTime`, Ctrl+C from `SetConsoleCtrlHandler`, and `CPUID` from
  `core::arch`, so no `chrono`, `ctrlc` or `libloading` is needed.
- **Alternatives considered**: `libloading` (redundant with `windows`), `chrono` (heavyweight for a
  single `HH:MM:SS`).

## R7. Reference for accuracy

- **Decision**: Compare against HWiNFO's "CPU (Tctl/Tdie)" sensor, logged to CSV. For the load
  phase, use any all-core stress tool (e.g. Cinebench multi-core).
- **Rationale**: SC-002 needs readings taken at the same moments, and both tools log once per
  second.
