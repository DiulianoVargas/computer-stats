<!--
Sync Impact Report
- Version change: 1.0.0 → 1.1.0
- Modified principles: III. Unsafe Isolation (FFI list: NVML → D3DKMT)
- Modified sections: Technical Constraints (GPU data source: NVML → Windows graphics statistics,
  ADR-0003)
- Rationale: NVML cost ≈ 19.5 MB private memory, leaving no headroom in Principle I's 30 MB budget
- Templates requiring updates: none (templates read the constitution at runtime)
- Follow-up TODOs: none
-->

# Computer Stats Constitution

Computer Stats is a lightweight Windows tray app (Rust + Slint) that shows Metrics and lets the
user adjust App Volumes. Domain terms are defined in `CONTEXT.md`; architectural decisions live in
`docs/adr/`.

## Core Principles

### I. Resource Budgets (NON-NEGOTIABLE)

- Private working set MUST stay ≤ 30 MB with the Panel closed and ≤ 45 MB with the Panel open.
- CPU usage MUST stay < 0.5% during background sampling and < 2% with the Panel open.
- The app MUST run as a single process.
- The release executable MUST be ≤ 10 MB.
- A `Get-Process`-based measurement script MUST run before every release; exceeding any budget
  blocks the release.

Rationale: "lightweight" is the product's core promise (ADR-0001) and erodes silently unless it
is measured.

### II. Native Only (NON-NEGOTIABLE)

- The app MUST NOT embed a web view, a managed/scripting runtime, or spawn sidecar processes.
- `cargo-deny` MUST ban `wry`, `webview2-com`, `tauri` and equivalent crates.

Rationale: a web view or sidecar alone would exceed the budgets in Principle I and reverse
ADR-0001.

### III. Unsafe Isolation (NON-NEGOTIABLE)

- All `unsafe` code and FFI (Core Audio COM, D3DKMT, PawnIO) MUST live in a single `platform`
  module behind safe APIs.
- Every other module MUST declare `#![deny(unsafe_code)]`.

Rationale: the app runs elevated (ADR-0002); memory-safety bugs must be confined to one
reviewable place.

### IV. Elevated Minimal Surface (NON-NEGOTIABLE)

- The app MUST NOT perform network access; `cargo-deny` MUST ban `reqwest`, `hyper`, `ureq` and
  equivalents.
- The app MUST NOT spawn processes, except `schtasks` to manage Start with Windows.
- The app MUST NOT load plugins or external code at runtime.
- App Volume MUST change only in response to direct user input in the Panel.

Rationale: an always-elevated process is a privilege-escalation target; every capability it
lacks is one that cannot be abused.

### V. Software Rendering

- Slint SHOULD use its software renderer with `default-features = false`, enabling only needed
  features.

Rationale: a GPU renderer costs RAM and would appear in the app's own VRAM and GPU Usage
readings. Deviation requires measured justification against Principle I.

### VI. Dependency Discipline

- Every new crate MUST have a one-line justification in the change that adds it, with default
  features disabled unless needed.
- `cargo-deny` MUST check advisories, duplicate versions, and licenses.
- Slint's royalty-free license attribution MUST be honored.

Rationale: dependencies are the main source of size, memory, and supply-chain risk.

### VII. Sampling Discipline

- Sampling MUST be timer-driven; busy loops are forbidden.
- Sensor reads MUST NOT run on the UI thread.
- Sparkline history MUST use fixed-size ring buffers so memory does not grow over time.

Rationale: a monitor must not become the load it measures.

### VIII. Graceful Degradation

- Each Metric source (CPU, GPU, PawnIO, Core Audio, Drives) MUST fail independently; a failed
  Metric shows `—` while the rest keep working.
- The sampling path MUST NOT panic.

Rationale: hardware and drivers vary (no NVIDIA GPU, PawnIO missing); one failure must not take
down the Panel.

### IX. Code Hygiene & Ubiquitous Language

- `cargo fmt --check` and `cargo clippy -- -D warnings` MUST pass.
- Code identifiers MUST use the terms in `CONTEXT.md` (e.g. `AppVolume`, `WarningLevel`,
  `StatusDot`); a new domain term MUST be added to the glossary before it appears in code.

Rationale: one vocabulary across conversation, specs, and code prevents drift and ambiguity.

## Technical Constraints

- Language and UI: Rust (stable) + Slint (ADR-0001).
- Platform: Windows 11, always elevated (ADR-0002).
- Data sources: Windows Core Audio COM via the `windows` crate (App Volume, Audio Level); Windows
  kernel graphics statistics, D3DKMT in `gdi32.dll` (GPU Usage, VRAM, GPU Temperature; ADR-0003);
  PawnIO driver (CPU Temperature); Windows system APIs (CPU Usage, Memory Usage, Disk Space).
- Release profile: `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`.

## Quality Gates

Every change MUST pass before merge:

1. `cargo fmt --check`
2. `cargo clippy -- -D warnings`
3. `cargo deny check` (bans, advisories, licenses, duplicates)
4. `cargo test`

Every release MUST additionally pass the resource-budget measurement script (Principle I).

## Governance

- This constitution supersedes other practices; specs, plans, and tasks MUST comply with it.
- Amendments require a documented rationale and, when they reverse an architectural decision, a
  new or superseding ADR in `docs/adr/`.
- Versioning: MAJOR for removing or redefining a principle; MINOR for adding a principle or
  materially expanding guidance; PATCH for clarifications.
- Plans MUST include a constitution check; any deviation MUST be justified in the plan's
  complexity tracking.

**Version**: 1.1.0 | **Ratified**: 2026-10-19 | **Last Amended**: 2026-10-20
