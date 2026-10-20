//! The UI thread: owns the Panel and the Tray Icon, receives Snapshots from the sampler.

use std::cell::RefCell;

use crate::domain::metric::Reason;
use crate::domain::tray_art::{self, TrayTheme};
use crate::generated::PanelWindow;
use crate::platform::theme;
use crate::platform::tray::{TrayEvent, TrayIcon};
use crate::sampler;
use crate::snapshot::Snapshot;

/// Everything the UI thread owns. Lives in a thread-local so callbacks queued from other threads
/// (`invoke_from_event_loop` requires `Send`) can reach it.
struct App {
    _panel: PanelWindow,
    tray: TrayIcon,
    latest: Snapshot,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with_borrow_mut(|app| app.as_mut().map(f))
}

/// Runs the app until Quit.
pub fn run() -> Result<(), String> {
    let panel = PanelWindow::new().map_err(|e| e.to_string())?;

    let tray = TrayIcon::create(on_tray_event)?;
    let theme = tray_theme();
    let size = tray.icon_size();
    tray.set_icon(&tray_art::render(size, theme), size);

    APP.with_borrow_mut(|app| {
        *app = Some(App {
            _panel: panel,
            tray,
            latest: Snapshot::unavailable(Reason::ReadFailed),
        })
    });

    let mut sampler = sampler::spawn(|snapshot| {
        let _ = slint::invoke_from_event_loop(move || on_snapshot(snapshot));
    })
    .map_err(|e| e.to_string())?;

    let result = slint::run_event_loop_until_quit().map_err(|e| e.to_string());

    sampler.stop();
    // Dropping the App removes the Tray Icon.
    APP.with_borrow_mut(|app| app.take());
    result
}

fn on_snapshot(snapshot: Snapshot) {
    with_app(|app| {
        app.tray.set_tooltip(&snapshot.tooltip);
        app.latest = snapshot;
    });
}

fn on_tray_event(event: TrayEvent) {
    match event {
        TrayEvent::Select => {}
        TrayEvent::ContextMenu { .. } => {}
    }
}

fn tray_theme() -> TrayTheme {
    if theme::taskbar_uses_light_theme() {
        TrayTheme::Light
    } else {
        TrayTheme::Dark
    }
}
