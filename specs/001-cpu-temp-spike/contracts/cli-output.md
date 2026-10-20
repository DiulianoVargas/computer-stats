# Contract: `cpu-temp-spike` console output

## Invocation

```text
cpu-temp-spike.exe            # no arguments
```

`AMDFamily17.bin` must sit next to the executable.

## Output: one line per second on stdout

```text
<HH:MM:SS>  <value>[  (<reason>)]
```

| Case | Example line |
|------|--------------|
| Reading OK | `14:02:07  54.3 °C` |
| Not elevated | `14:02:07  —  (not elevated)` |
| Driver missing | `14:02:07  —  (driver not found)` |
| Unsupported CPU | `14:02:07  —  (unsupported CPU)` |
| Transient failure / implausible | `14:02:07  —  (read failed)` |

- The first line appears within 2 seconds of start.
- The value is always one decimal place, followed by a space and `°C`.
- The separator between fields is two spaces.

## Exit

| Trigger | Exit code |
|---------|-----------|
| Ctrl+C / Ctrl+Break / console close | `0` |

The program never exits on its own and never panics (FR-004).
