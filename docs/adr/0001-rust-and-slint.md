# Rust + Slint, no web view

Lowest possible RAM is the top non-functional goal, so the app is written in Rust with a natively rendered Slint UI (~15–30 MB expected). Tauri was rejected despite its "lightweight" reputation: that refers to installer size, while its WebView2 runtime spawns several Edge processes (~80–150 MB), and CPU temperature would have required a separate .NET sidecar on top. C# + WPF (~40–60 MB) was the easier path but heavier.

## Consequences

- CPU Temperature cannot use LibreHardwareMonitorLib (.NET); it must be read from Rust, talking to a kernel driver directly (see ADR-0002).
- Per-app audio uses the Windows Core Audio COM APIs via the `windows` crate; GPU Metrics use NVML.
