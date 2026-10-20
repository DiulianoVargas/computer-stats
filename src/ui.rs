//! The UI thread: owns the Panel and the Tray Icon, receives Snapshots from the sampler.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use slint::winit_030::winit::event::WindowEvent;
use slint::winit_030::winit::platform::windows::{CornerPreference, WindowAttributesExtWindows};
use slint::winit_030::{EventResult, WinitWindowAccessor};
use slint::{ComponentHandle, ModelRc, PhysicalPosition, SharedString, VecModel};

use crate::crash_log;
use crate::desktop_widget::DesktopWidget;
use crate::domain::anchor::{self, Size};
use crate::domain::app_order::Sort;
use crate::domain::format;
use crate::domain::metric::{MetricKind, Reading, Reason};
use crate::domain::sparkline;
use crate::domain::tray_art::{self, TrayTheme};
use crate::domain::warning::WarningLevel;
use crate::generated::{DriveRowData, MetricRowData, PanelWindow};
use crate::platform::desktop_window::WidgetEvent;
use crate::platform::instance::{self, Instance};
use crate::platform::theme;
use crate::platform::tray::{self, MenuEntry, TrayEvent, TrayIcon};
use crate::platform::{settings, shell};
use crate::processes_ui::{self, EndResult, ProcessesUi};
use crate::sampler::{self, SamplerHandle};
use crate::snapshot::{DriveState, MetricState, Snapshot};
use crate::startup;

/// A click on the Tray Icon this soon after the Panel lost focus is the click that closed it
/// (the taskbar took focus first), so it must not reopen it (research R4).
const TOGGLE_GUARD: Duration = Duration::from_millis(300);

const MENU_START_WITH_WINDOWS: u32 = 1;
const MENU_QUIT: u32 = 2;
const MENU_DESKTOP_WIDGET: u32 = 3;

/// Logical width, as in `ui/panel.slint`.
const PANEL_WIDTH: f32 = 320.0;

/// The Panel's Views; values match `view` in `ui/panel.slint`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Metrics = 0,
    Processes = 1,
}

/// Everything the UI thread owns. Lives in a thread-local so callbacks queued from other threads
/// (`invoke_from_event_loop` requires `Send`) can reach it.
struct App {
    panel: PanelWindow,
    tray: TrayIcon,
    latest: Snapshot,
    /// What the Tray Icon currently shows, so it is redrawn only on change.
    icon: (WarningLevel, TrayTheme, u32),
    visible: bool,
    hidden_by_deactivation: Option<Instant>,
    /// Logical height the Panel was last positioned for.
    placed_height: f32,
    /// Start with Windows as last read from or written to Task Scheduler (FR-005).
    start_with_windows: bool,
    sampler: SamplerHandle,
    /// The selected View, remembered while the app runs (003 FR-002).
    view: View,
    processes: ProcessesUi,
    /// The Desktop Widget, only while the user has it on (005 FR-001).
    desktop_widget: Option<DesktopWidget>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` on the App and returns its result, or `None` if the App is already borrowed.
///
/// Win32 and Slint calls made while the App is borrowed (e.g. `Shell_NotifyIconW`, showing the
/// Panel, reading a Slint property) can deliver window messages or Slint callbacks synchronously,
/// which re-enter this module. A second `borrow_mut` would panic (and `panic = "abort"` kills the
/// app), so re-entrant access is refused instead and noted in the crash log.
#[track_caller]
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    let caller = std::panic::Location::caller();
    APP.with(|cell| match cell.try_borrow_mut() {
        Ok(mut app) => app.as_mut().map(f),
        Err(_) => {
            note_reentry(caller);
            None
        }
    })
}

/// Runs `f` on the App now, or, if the App is already borrowed (see `with_app`), right after the
/// current work finishes, on the same UI thread. For events whose effect must not be lost.
#[track_caller]
fn update_app(f: impl FnOnce(&mut App) + 'static) {
    let caller = std::panic::Location::caller();
    let busy = APP.with(|cell| cell.try_borrow_mut().is_err());
    if busy {
        note_reentry(caller);
        slint::Timer::single_shot(Duration::ZERO, move || update_app(f));
    } else {
        with_app(f);
    }
}

/// Records each re-entrant call site once per run, for diagnosis.
fn note_reentry(caller: &'static std::panic::Location<'static>) {
    thread_local! {
        static SEEN: RefCell<Vec<&'static std::panic::Location<'static>>> =
            const { RefCell::new(Vec::new()) };
    }
    let first = SEEN.with(|seen| match seen.try_borrow_mut() {
        Ok(mut seen) if !seen.contains(&caller) => {
            seen.push(caller);
            true
        }
        _ => false,
    });
    if first {
        crash_log::write(&format!(
            "re-entrant UI access from {caller}; deferred or skipped"
        ));
    }
}

/// Runs the app until Quit.
pub fn run() -> Result<(), String> {
    // A second launch opens the running instance's Panel and exits (FR-023).
    let _instance = match instance::acquire() {
        Instance::First(guard) => guard,
        Instance::AlreadyRunning => return Ok(()),
    };

    let tray = TrayIcon::create(on_tray_event)?;
    let icon = (WarningLevel::Normal, tray_theme(), tray.icon_size());
    tray.set_icon(&tray_art::render(icon.2, icon.1, icon.0), icon.2);

    // Owned by the hidden tray window: no taskbar button and not in Alt+Tab (FR-001).
    let owner = tray.hwnd();
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(move |attributes| {
            attributes
                .with_owner_window(owner)
                .with_skip_taskbar(true)
                .with_corner_preference(CornerPreference::Round)
        })
        .select()
        .map_err(|e| e.to_string())?;

    let panel = PanelWindow::new().map_err(|e| e.to_string())?;
    panel.on_close_requested(|| {
        update_app(move |app| app.hide());
    });
    panel.on_view_changed(|view| {
        update_app(move |app| {
            app.set_view(if view == 1 {
                View::Processes
            } else {
                View::Metrics
            })
        });
    });
    panel.on_sort_clicked(|column| {
        update_app(move |app| {
            let sort = if column == 1 { Sort::Memory } else { Sort::Cpu };
            app.processes.set_sort(sort);
            app.panel.set_sort(column);
        });
    });
    panel.on_end_clicked(|key| {
        let result = with_app(|app| app.processes.end_clicked(&key));
        if let Some(EndResult::Quit) = result {
            let _ = slint::quit_event_loop();
            return;
        }
        // Expire arming and row feedback later (FR-012, FR-016).
        slint::Timer::single_shot(processes_ui::expire_after(), || {
            update_app(move |app| app.processes.expire());
        });
    });
    // Re-anchor as soon as the height changes, not at the next Snapshot (no visible jump).
    panel.on_panel_height_changed(|| {
        update_app(move |app| {
            if app.visible {
                app.place_if_resized();
            }
        });
    });
    panel.on_background_clicked(|| {
        update_app(move |app| app.processes.cancel_arming());
    });
    // Click outside: the Panel loses focus (FR-008). Handled after winit's dispatch returns.
    panel.window().on_winit_window_event(|_, event| {
        if let WindowEvent::Focused(false) = event {
            let _ = slint::invoke_from_event_loop(|| {
                update_app(move |app| {
                    if app.visible {
                        app.hide();
                        app.hidden_by_deactivation = Some(Instant::now());
                    }
                });
            });
        }
        EventResult::Propagate
    });

    let sampler = sampler::spawn(|snapshot| {
        let _ = slint::invoke_from_event_loop(move || on_snapshot(snapshot));
    })
    .map_err(|e| e.to_string())?;

    let processes = ProcessesUi::new();
    panel.set_apps(processes.model());

    APP.with_borrow_mut(|app| {
        *app = Some(App {
            panel,
            tray,
            latest: Snapshot::unavailable(Reason::ReadFailed),
            icon,
            visible: false,
            hidden_by_deactivation: None,
            placed_height: 0.0,
            start_with_windows: startup::is_enabled(),
            sampler,
            view: initial_view(),
            processes,
            desktop_widget: None,
        })
    });

    with_app(|app| app.panel.set_view(app.view as i32));

    // Restore the Desktop Widget as it was at quit (005 FR-007), once the event loop runs.
    if settings::load().shown {
        slint::Timer::single_shot(Duration::ZERO, || {
            update_app(move |app| app.set_desktop_widget(true));
        });
    }

    let result = slint::run_event_loop_until_quit().map_err(|e| e.to_string());

    // Dropping the App stops the sampler and removes the Tray Icon.
    if let Some(mut app) = APP.with_borrow_mut(|app| app.take()) {
        app.sampler.stop();
    }
    result
}

fn on_snapshot(mut snapshot: Snapshot) {
    update_app(move |app| {
        app.tray.set_tooltip(&snapshot.tooltip);
        if let Some(sample) = snapshot.processes.take()
            && app.visible
            && app.view == View::Processes
        {
            let hovered = app.panel.get_apps_hovered();
            app.processes.apply(sample, hovered);
            app.panel.set_apps_loading(false);
        }
        app.latest = snapshot;
        app.update_icon();
        // Live whenever it is on, Panel open or not (005 FR-003).
        if let Some(widget) = app.desktop_widget.as_mut() {
            widget.update(&app.latest);
        }
        // Panel properties change only while it is visible (Constitution I).
        if app.visible {
            app.push_snapshot();
            app.place_if_resized();
        }
    });
}

fn on_tray_event(event: TrayEvent) {
    match event {
        TrayEvent::Select => {
            update_app(move |app| app.toggle());
        }
        TrayEvent::OpenPanel => {
            update_app(move |app| {
                if !app.visible {
                    app.open();
                }
            });
        }
        TrayEvent::AppearanceChanged => {
            update_app(move |app| {
                let wanted = (app.icon.0, tray_theme(), app.tray.icon_size());
                app.update_icon_to(wanted);
                if let Some(widget) = app.desktop_widget.as_mut() {
                    widget.set_dark(!theme::apps_use_light_theme());
                }
            });
        }
        TrayEvent::ContextMenu { x, y } => {
            let (checked, widget_shown) =
                with_app(|app| (app.start_with_windows, app.desktop_widget.is_some()))
                    .unwrap_or((false, false));
            // No App borrow across the menu's modal loop: snapshots keep arriving meanwhile.
            let chosen = tray::show_menu(
                x,
                y,
                &[
                    MenuEntry::Item {
                        id: MENU_START_WITH_WINDOWS,
                        text: "Start with Windows",
                        checked: Some(checked),
                    },
                    MenuEntry::Item {
                        id: MENU_DESKTOP_WIDGET,
                        text: "Show desktop widget",
                        checked: Some(widget_shown),
                    },
                    MenuEntry::Separator,
                    MenuEntry::Item {
                        id: MENU_QUIT,
                        text: "Quit",
                        checked: None,
                    },
                ],
            );
            match chosen {
                Some(MENU_START_WITH_WINDOWS) => {
                    let result = if checked {
                        startup::disable()
                    } else {
                        startup::enable()
                    };
                    // On failure the checkmark keeps the old state; no dialog (contracts/ui.md).
                    if result.is_ok() {
                        update_app(move |app| app.start_with_windows = !checked);
                    }
                }
                Some(MENU_DESKTOP_WIDGET) => {
                    update_app(move |app| {
                        app.set_desktop_widget(!widget_shown);
                        // The check mark shows the actual state (contracts/ui.md).
                        settings::save_shown(app.desktop_widget.is_some());
                    });
                }
                Some(MENU_QUIT) => {
                    let _ = slint::quit_event_loop();
                }
                _ => {}
            }
        }
    }
}

fn on_widget_event(event: WidgetEvent) {
    update_app(move |app| {
        if let Some(widget) = app.desktop_widget.as_mut() {
            widget.on_event(event);
        }
    });
}

impl App {
    /// Turns the Desktop Widget on or off. If it cannot be created it stays off; the tray and
    /// Panel are unaffected (005 FR-013).
    fn set_desktop_widget(&mut self, on: bool) {
        if on == self.desktop_widget.is_some() {
            return;
        }
        if !on {
            self.desktop_widget = None;
            return;
        }
        let saved = settings::load().position;
        match DesktopWidget::new(saved, !theme::apps_use_light_theme(), on_widget_event) {
            Ok(mut widget) => {
                widget.show(&self.latest);
                self.desktop_widget = Some(widget);
            }
            Err(e) => crash_log::write(&format!("desktop widget failed: {e}")),
        }
    }

    fn toggle(&mut self) {
        if self.visible {
            self.hide();
        } else if self
            .hidden_by_deactivation
            .is_some_and(|at| at.elapsed() < TOGGLE_GUARD)
        {
            // This click is what closed the Panel; do not reopen it.
        } else {
            self.open();
        }
    }

    /// Fills the Panel with the latest values, places it next to the Tray Icon and shows it with
    /// focus, so the first frame already has values (SC-001).
    fn open(&mut self) {
        self.push_snapshot();
        self.place();
        if self.panel.show().is_err() {
            return;
        }
        self.visible = true;
        self.hidden_by_deactivation = None;
        if self.view == View::Processes {
            self.sampler.set_processes_visible(true);
        }
        // The winit window, and with it the Drive rows' height, may only exist after this
        // event-loop turn; re-place and focus it then.
        let _ = slint::invoke_from_event_loop(|| {
            update_app(move |app| {
                app.place_if_resized();
                app.panel.window().with_winit_window(|w| w.focus_window());
            });
        });
    }

    /// Positions the Panel for its current height (FR-007). The height changes when Drives
    /// appear or disappear, and the window grows downward, so it is re-placed then.
    fn place(&mut self) {
        let height = self.panel.get_panel_height();
        let icon = self.tray.rect().unwrap_or_else(shell::cursor_rect);
        let scale = shell::scale_for(icon);
        let size = Size {
            width: (PANEL_WIDTH * scale).round() as i32,
            height: (height * scale).round() as i32,
        };
        if let Some(work_area) = shell::work_area_for(icon) {
            let at = anchor::place(icon, work_area, shell::taskbar_edge(), size);
            self.panel
                .window()
                .set_position(PhysicalPosition::new(at.x, at.y));
        }
        self.placed_height = height;
    }

    fn place_if_resized(&mut self) {
        if self.panel.get_panel_height() != self.placed_height {
            self.place();
        }
    }

    /// Redraws the Tray Icon only when the Status Dot changes (FR-017, SC-005).
    fn update_icon(&mut self) {
        let wanted = (self.latest.status_dot.level, self.icon.1, self.icon.2);
        self.update_icon_to(wanted);
    }

    /// Redraws only when the level, taskbar theme or icon size differs from what is shown.
    fn update_icon_to(&mut self, wanted: (WarningLevel, TrayTheme, u32)) {
        if wanted != self.icon {
            self.tray
                .set_icon(&tray_art::render(wanted.2, wanted.1, wanted.0), wanted.2);
            self.icon = wanted;
        }
    }

    fn hide(&mut self) {
        let _ = self.panel.hide();
        self.visible = false;
        self.stop_processes();
    }

    /// Switches View (003 FR-001): Processes are sampled only while their View is visible.
    fn set_view(&mut self, view: View) {
        self.view = view;
        self.panel.set_view(view as i32);
        if view == View::Processes {
            self.sampler.set_processes_visible(true);
        } else {
            self.stop_processes();
        }
        self.place_if_resized();
    }

    /// Stops Processes sampling and drops the View's data (FR-018); arming is cancelled.
    fn stop_processes(&mut self) {
        self.sampler.set_processes_visible(false);
        self.processes.clear();
        self.panel.set_apps_loading(true);
        self.panel.set_sort(match self.processes.sort() {
            Sort::Cpu => 0,
            Sort::Memory => 1,
        });
    }

    fn push_snapshot(&self) {
        let m = &self.latest.metrics;
        let row = |kind: MetricKind| metric_row(&m[kind.index()]);
        self.panel.set_cpu_usage(row(MetricKind::CpuUsage));
        self.panel
            .set_cpu_temperature(row(MetricKind::CpuTemperature));
        self.panel.set_gpu_usage(row(MetricKind::GpuUsage));
        self.panel
            .set_gpu_temperature(row(MetricKind::GpuTemperature));
        self.panel.set_vram(row(MetricKind::Vram));
        self.panel.set_memory_usage(row(MetricKind::MemoryUsage));
        let drives: Vec<DriveRowData> = self.latest.drives.iter().map(drive_row).collect();
        self.panel.set_drives(ModelRc::new(VecModel::from(drives)));
    }
}

/// The row the Panel and the Desktop Widget show for one Metric (005 SC-005).
pub(crate) fn metric_row(state: &MetricState) -> MetricRowData {
    let label = match state.kind {
        MetricKind::CpuUsage | MetricKind::GpuUsage | MetricKind::MemoryUsage => "Usage",
        MetricKind::CpuTemperature | MetricKind::GpuTemperature => "Temperature",
        MetricKind::Vram => "VRAM",
    };
    let (value, secondary, unavailable) = match state.reading {
        Reading::Unavailable(reason) => (format::unavailable(reason), String::new(), true),
        Reading::Value {
            value,
            used_bytes: Some(used),
            total_bytes: Some(total),
        } => (
            format::used_total(used, total),
            format::percent(value),
            false,
        ),
        Reading::Value { value, .. } if state.kind.is_temperature() => {
            (format::temperature(value), String::new(), false)
        }
        Reading::Value { value, .. } => (format::percent(value), String::new(), false),
    };
    MetricRowData {
        label: label.into(),
        value_text: value.into(),
        secondary_text: secondary.into(),
        unavailable,
        level: level_index(state.level),
        sparkline: {
            let (min, max) = sparkline::scale(state.kind);
            sparkline::path(&state.history, min, max).into()
        },
    }
}

fn drive_row(state: &DriveState) -> DriveRowData {
    match state {
        DriveState::Ready { drive, level } => DriveRowData {
            label_text: format::drive_label(drive.letter, &drive.label).into(),
            free_text: format::drive_free(drive.free_bytes, drive.total_bytes).into(),
            used_fraction: drive.used_pct() / 100.0,
            unavailable: false,
            level: level_index(Some(*level)),
        },
        DriveState::Unavailable { letter, reason } => DriveRowData {
            label_text: SharedString::from(format!("{letter}:")),
            free_text: format::unavailable(*reason).into(),
            used_fraction: 0.0,
            unavailable: true,
            level: 0,
        },
    }
}

/// The `level` int the Slint rows expect.
fn level_index(level: Option<WarningLevel>) -> i32 {
    match level {
        Some(WarningLevel::Critical) => 2,
        Some(WarningLevel::Warning) => 1,
        _ => 0,
    }
}

/// Metrics at launch (003 FR-001). Debug builds accept `COMPUTER_STATS_TEST_VIEW=processes` to
/// open on the Processes View, for manual tests without clicking the tab.
fn initial_view() -> View {
    if cfg!(debug_assertions)
        && std::env::var("COMPUTER_STATS_TEST_VIEW").is_ok_and(|v| v == "processes")
    {
        View::Processes
    } else {
        View::Metrics
    }
}

fn tray_theme() -> TrayTheme {
    if theme::taskbar_uses_light_theme() {
        TrayTheme::Light
    } else {
        TrayTheme::Dark
    }
}
