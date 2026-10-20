//! The Desktop Widget (specs/005-desktop-widget): renders `WidgetWindow` off-screen once per
//! Snapshot and presents it on the layered `DesktopWindow` (research R1). Exists only while the
//! user has it on.

use slint::{ComponentHandle, PhysicalSize};

use crate::domain::anchor::{Point, Rect, Size};
use crate::domain::metric::MetricKind;
use crate::domain::widget_place;
use crate::generated::WidgetWindow;
use crate::platform::desktop_window::{DesktopWindow, WidgetEvent};
use crate::platform::{settings, shell};
use crate::snapshot::{FansState, Snapshot};
use crate::ui::{fan_row, metric_row};

/// Logical width, as in `ui/widget.slint`.
const WIDGET_WIDTH: f32 = 320.0;
/// Logical distance from the work area's corner when first shown (contracts/ui.md).
const DEFAULT_INSET: f32 = 24.0;

pub struct DesktopWidget {
    component: WidgetWindow,
    window: DesktopWindow,
    /// The scale the content is rendered at: the widget's monitor (FR-009).
    scale: f32,
    top_left: Point,
    /// `top_left` changed and the window must move there at the next paint. Otherwise the window
    /// stays where it is, so a Snapshot during a drag does not pull it back.
    reposition: bool,
}

impl DesktopWidget {
    /// Creates the widget hidden, at `saved` (kept on a visible monitor) or, if never placed, at
    /// the primary monitor's top-right corner.
    pub fn new(
        saved: Option<Point>,
        dark: bool,
        on_event: impl Fn(WidgetEvent) + 'static,
    ) -> Result<DesktopWidget, String> {
        let component = WidgetWindow::new().map_err(|e| e.to_string())?;
        component.set_dark(dark);
        let window = DesktopWindow::create(on_event)?;
        let mut widget = DesktopWidget {
            component,
            window,
            scale: 1.0,
            top_left: Point { x: 0, y: 0 },
            reposition: true,
        };
        match saved {
            Some(p) => {
                widget.top_left = p;
                widget.scale = shell::scale_for(widget.rect());
                widget.keep_on_screen();
            }
            None => {
                let area = shell::primary_work_area().ok_or("no primary monitor")?;
                widget.scale = shell::scale_for(area);
                let inset = (DEFAULT_INSET * widget.scale).round() as i32;
                widget.top_left = widget_place::default_position(area, widget.size(), inset);
            }
        }
        Ok(widget)
    }

    /// Fills the widget with `snapshot` and shows it behind every window.
    pub fn show(&mut self, snapshot: &Snapshot) {
        self.update(snapshot);
        self.window.show();
    }

    /// The same rows as the Panel for the Metrics it shows (FR-002, SC-005), then repaints. Fans:
    /// the GPU Fan, and the CPU Fan only on a known board (006 FR-011).
    pub fn update(&mut self, snapshot: &Snapshot) {
        let m = &snapshot.metrics;
        let row = |kind: MetricKind| metric_row(&m[kind.index()]);
        self.component.set_cpu_usage(row(MetricKind::CpuUsage));
        self.component
            .set_cpu_temperature(row(MetricKind::CpuTemperature));
        self.component.set_gpu_usage(row(MetricKind::GpuUsage));
        self.component
            .set_gpu_temperature(row(MetricKind::GpuTemperature));
        self.component.set_gpu_fan(row(MetricKind::GpuFan));
        let cpu_fan = match &snapshot.fans {
            FansState::Fans(fans) => fans.iter().find(|f| f.cpu_fan),
            FansState::Unavailable(_) => None,
        };
        self.component.set_has_cpu_fan(cpu_fan.is_some());
        if let Some(fan) = cpu_fan {
            self.component.set_cpu_fan(fan_row(fan));
        }
        self.component
            .set_memory_usage(row(MetricKind::MemoryUsage));
        self.paint();
    }

    /// Follows the Windows light/dark mode (US3).
    pub fn set_dark(&mut self, dark: bool) {
        if self.component.get_dark() != dark {
            self.component.set_dark(dark);
            self.paint();
        }
    }

    pub fn on_event(&mut self, event: WidgetEvent) {
        // During a drag the window has moved without us: start from where it is now.
        if let Some(r) = self.window.rect() {
            self.top_left = Point {
                x: r.left,
                y: r.top,
            };
        }
        match event {
            WidgetEvent::Moved(p) => {
                self.top_left = p;
                // Dropped on a monitor with other scaling, with no WM_DPICHANGED on the way.
                self.scale = shell::scale_for(self.rect());
                self.keep_on_screen();
                self.paint();
                settings::save_position(self.top_left);
            }
            WidgetEvent::DpiChanged(scale) => {
                self.scale = scale;
                self.keep_on_screen();
                self.paint();
            }
            WidgetEvent::DisplayChanged => {
                self.keep_on_screen();
                self.paint();
            }
        }
    }

    /// Pulls the widget fully into the work area of its nearest monitor (FR-006, FR-008).
    fn keep_on_screen(&mut self) {
        if let Some(area) = shell::work_area_for(self.rect()) {
            let clamped = widget_place::clamp(self.top_left, self.size(), area);
            if clamped != self.top_left {
                self.top_left = clamped;
                self.reposition = true;
            }
        }
    }

    /// Renders the content at `scale` and puts it on screen, moving it to `top_left` only when
    /// `reposition` is set. A failed render skips this frame; the next Snapshot tries again.
    fn paint(&mut self) {
        let window = self.component.window();
        if window.scale_factor() != self.scale {
            window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
                scale_factor: self.scale,
            });
        }
        let size = self.size();
        let physical = PhysicalSize::new(size.width as u32, size.height as u32);
        if window.size() != physical {
            window.set_size(physical);
        }
        if let Ok(image) = window.take_snapshot() {
            let move_to = std::mem::take(&mut self.reposition).then_some(self.top_left);
            self.window
                .present(image.as_bytes(), image.width(), image.height(), move_to);
        }
    }

    /// Physical size at the current scale.
    fn size(&self) -> Size {
        Size {
            width: (WIDGET_WIDTH * self.scale).round() as i32,
            height: (self.component.get_widget_height() * self.scale).round() as i32,
        }
    }

    fn rect(&self) -> Rect {
        let size = self.size();
        Rect {
            left: self.top_left.x,
            top: self.top_left.y,
            right: self.top_left.x + size.width,
            bottom: self.top_left.y + size.height,
        }
    }
}

impl Drop for DesktopWidget {
    fn drop(&mut self) {
        self.window.hide();
    }
}
