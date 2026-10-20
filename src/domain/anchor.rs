//! Panel placement next to the Tray Icon (research R5; FR-007). All values in physical pixels.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    fn center_x(&self) -> i32 {
        (self.left + self.right) / 2
    }
    fn center_y(&self) -> i32 {
        (self.top + self.bottom) / 2
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// The screen edge the taskbar is docked to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Bottom,
    Top,
    Left,
    Right,
}

/// Gap between the Panel and the taskbar, and between the Panel and the work-area edges.
pub const MARGIN: i32 = 12;

/// Top-left corner for a Panel of `size`: `MARGIN` away from the taskbar, centered on the icon along
/// the taskbar, and clamped inside `work_area` (which excludes the taskbar) with `MARGIN`.
pub fn place(icon: Rect, work_area: Rect, edge: Edge, size: Size) -> Point {
    let (x, y) = match edge {
        Edge::Bottom => (
            icon.center_x() - size.width / 2,
            work_area.bottom - MARGIN - size.height,
        ),
        Edge::Top => (icon.center_x() - size.width / 2, work_area.top + MARGIN),
        Edge::Left => (work_area.left + MARGIN, icon.center_y() - size.height / 2),
        Edge::Right => (
            work_area.right - MARGIN - size.width,
            icon.center_y() - size.height / 2,
        ),
    };
    Point {
        x: clamp(
            x,
            work_area.left + MARGIN,
            work_area.right - MARGIN - size.width,
        ),
        y: clamp(
            y,
            work_area.top + MARGIN,
            work_area.bottom - MARGIN - size.height,
        ),
    }
}

/// Like `i32::clamp`, but a Panel larger than the space sticks to the start instead of panicking.
fn clamp(v: i32, min: i32, max: i32) -> i32 {
    if max < min { min } else { v.clamp(min, max) }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1920×1080 screen.
    const SIZE: Size = Size {
        width: 340,
        height: 400,
    };

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn bottom_taskbar_centers_above_icon() {
        let work = rect(0, 0, 1920, 1032);
        let icon = rect(1500, 1040, 1524, 1064);
        let p = place(icon, work, Edge::Bottom, SIZE);
        assert_eq!(
            p,
            Point {
                x: 1512 - 170,
                y: 1032 - 12 - 400
            }
        );
    }

    #[test]
    fn icon_at_far_right_is_clamped() {
        let work = rect(0, 0, 1920, 1032);
        let icon = rect(1880, 1040, 1904, 1064);
        let p = place(icon, work, Edge::Bottom, SIZE);
        assert_eq!(p.x, 1920 - 12 - 340);
    }

    #[test]
    fn top_taskbar() {
        let work = rect(0, 48, 1920, 1080);
        let icon = rect(1500, 12, 1524, 36);
        let p = place(icon, work, Edge::Top, SIZE);
        assert_eq!(p, Point { x: 1342, y: 60 });
    }

    #[test]
    fn left_taskbar() {
        let work = rect(48, 0, 1920, 1080);
        let icon = rect(12, 900, 36, 924);
        let p = place(icon, work, Edge::Left, SIZE);
        assert_eq!(
            p,
            Point {
                x: 60,
                y: 1080 - 12 - 400
            }
        );
    }

    #[test]
    fn right_taskbar() {
        let work = rect(0, 0, 1872, 1080);
        let icon = rect(1884, 500, 1908, 524);
        let p = place(icon, work, Edge::Right, SIZE);
        assert_eq!(
            p,
            Point {
                x: 1872 - 12 - 340,
                y: 512 - 200
            }
        );
    }

    #[test]
    fn cursor_rect_fallback() {
        let work = rect(0, 0, 1920, 1032);
        let cursor = rect(800, 1050, 801, 1051);
        let p = place(cursor, work, Edge::Bottom, SIZE);
        assert_eq!(
            p,
            Point {
                x: 800 - 170,
                y: 620
            }
        );
    }

    #[test]
    fn panel_larger_than_work_area_does_not_panic() {
        let work = rect(0, 0, 300, 300);
        let p = place(rect(10, 10, 20, 20), work, Edge::Bottom, SIZE);
        assert_eq!(p, Point { x: 12, y: 12 });
    }
}
