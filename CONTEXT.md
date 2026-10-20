# Computer Stats

A lightweight, easy-to-read visualizer for a Windows PC's resource usage and per-app audio settings.

## Language

### General

**App**:
A Windows program running on this PC that the visualizer can report on, identified by its program file. A program with several audio sessions or several Processes (e.g. a browser) is still one App. In the Processes View this includes Windows' own programs and services.
_Avoid_: APK, application package, process (unless meaning the OS process specifically)

**Tray Icon**:
The visualizer's icon in the Windows system tray (next to the clock). It is the app's only permanent presence on screen.
_Avoid_: Taskbar icon, task manager icon

**Tray Menu**:
The small right-click menu on the Tray Icon holding app-level options (Start with Windows, Quit). Never shows Metrics.
_Avoid_: Context menu, settings

**Start with Windows**:
The user's choice to have the app launch automatically, already elevated, at login. Off by default.
_Avoid_: Autostart, startup, run at boot

**Status Dot**:
The colored dot drawn on the Tray Icon showing the worst Warning Level across all Metrics. Disk Space only affects it at Critical.
_Avoid_: Badge, indicator

**Panel**:
The popup that opens anchored above the Tray Icon on left-click and closes when the user clicks anywhere else. It has two Views, switched with tabs at its top: the Metrics View and the Processes View.
_Avoid_: Widget, window, dashboard, menu, flyout

**Metrics View**:
The Panel's default View: all Metrics and App Volumes.
_Avoid_: Main tab, home

**Processes View**:
The Panel's second View: every running App with its CPU and memory use, where the user can End an App.
_Avoid_: Task manager, process list, details

**Process**:
One running OS process (one PID). An App is made of one or more Processes started from the same program file. Shown to the user only as a count on an App.
_Avoid_: Task, job

**End App**:
Forcibly stopping every Process of an App from the Processes View, after the user confirms in place. Unsaved work in that App is lost.
_Avoid_: Kill, terminate (in user-facing text)

### Metrics

**Metric**:
A value the visualizer observes but does not change (CPU usage, memory, disk space, temperature, VRAM). Contrast with App Volume, which the user controls.
_Avoid_: Stat (when a single value is meant)

**CPU Usage** / **GPU Usage**:
How busy the processor / GPU is right now, as a percentage of its total capacity.
_Avoid_: Load, utilization (pick one: "Usage")

**Memory Usage**:
How much of the PC's system RAM is in use. Never includes VRAM.
_Avoid_: RAM usage, memory (bare)

**GPU**:
The PC's dedicated graphics card. Integrated graphics are ignored.
_Avoid_: Graphics, iGPU (for this term)

**VRAM**:
The GPU's own on-board memory. Never includes shared system memory.
_Avoid_: GPU memory, video memory

**Drive**:
A fixed, internal disk volume with a drive letter (e.g. C:). Removable USB drives are not Drives.
_Avoid_: Disk, partition, volume

**Disk Space**:
How much of a Drive's capacity is used versus free. Does not include read/write activity.
_Avoid_: Disk usage (ambiguous with activity)

**CPU Temperature** / **GPU Temperature**:
The current temperature of the processor and of the GPU respectively.
_Avoid_: Temp, heat

**Sparkline**:
A tiny line graph next to a Metric showing its last ~60 seconds. History is kept only while the app runs. Disk Space has no Sparkline.
_Avoid_: Graph, chart, history

**Warning Level**:
How concerning a Metric's current value is: Normal, Warning (amber) or Critical (red), decided by fixed thresholds. Shown by color only, never by notifications. CPU Usage and GPU Usage are always Normal.
_Avoid_: Alert, alarm, notification

### Audio

**App Volume**:
The volume setting Windows assigns to a single App in its volume mixer, separate from the master volume. The user can adjust it from the visualizer.
_Avoid_: Sound, app sound

**Audio Level**:
How loud an App's output is at this instant (a live meter), independent of its App Volume.
_Avoid_: Volume, sound

**Audio App**:
An App that has an audio session with Windows, whether or not it is currently making sound. Only Audio Apps appear in the volume list.

**Playing**:
An Audio App whose Audio Level is currently above silence. Playing apps are sorted to the top and highlighted.
_Avoid_: Active (overloaded)
