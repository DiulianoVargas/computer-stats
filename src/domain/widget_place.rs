//! Desktop Widget placement (005 data-model.md; research R6). All values in physical pixels.

use crate::domain::anchor::{Point, Rect, Size};

/// The remembered Desktop Widget state (FR-007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WidgetSettings {
    pub shown: bool,
    /// Top-left last placed by the user; `None` means never placed.
    pub position: Option<Point>,
}

/// The top-right corner of `work_area`, `inset` pixels from its top and right edges.
pub fn default_position(work_area: Rect, size: Size, inset: i32) -> Point {
    clamp(
        Point {
            x: work_area.right - inset - size.width,
            y: work_area.top + inset,
        },
        size,
        work_area,
    )
}

/// The closest position that keeps the whole rectangle inside `work_area` (FR-006, FR-008). If
/// `size` is larger than the work area, top-left is pinned to the work area's top-left.
pub fn clamp(top_left: Point, size: Size, work_area: Rect) -> Point {
    let axis = |pos: i32, len: i32, start: i32, end: i32| {
        if len >= end - start {
            start
        } else {
            pos.clamp(start, end - len)
        }
    };
    Point {
        x: axis(top_left.x, size.width, work_area.left, work_area.right),
        y: axis(top_left.y, size.height, work_area.top, work_area.bottom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1032,
    };
    const SIZE: Size = Size {
        width: 280,
        height: 190,
    };

    fn p(x: i32, y: i32) -> Point {
        Point { x, y }
    }

    #[test]
    fn default_is_top_right_inset() {
        assert_eq!(default_position(AREA, SIZE, 24), p(1920 - 24 - 280, 24));
    }

    #[test]
    fn default_on_a_monitor_left_of_the_primary() {
        let left = Rect {
            left: -2560,
            top: 0,
            right: 0,
            bottom: 1400,
        };
        assert_eq!(default_position(left, SIZE, 36), p(-36 - 280, 36));
    }

    #[test]
    fn default_without_inset_touches_the_corner() {
        assert_eq!(default_position(AREA, SIZE, 0), p(1920 - 280, 0));
    }

    #[test]
    fn inside_is_unchanged() {
        assert_eq!(clamp(p(100, 200), SIZE, AREA), p(100, 200));
    }

    #[test]
    fn off_each_edge_is_pulled_in() {
        assert_eq!(clamp(p(-50, 200), SIZE, AREA), p(0, 200));
        assert_eq!(clamp(p(1800, 200), SIZE, AREA), p(1920 - 280, 200));
        assert_eq!(clamp(p(100, -10), SIZE, AREA), p(100, 0));
        assert_eq!(clamp(p(100, 1000), SIZE, AREA), p(100, 1032 - 190));
    }

    #[test]
    fn fully_off_screen_lands_on_the_work_area() {
        // Saved on a monitor that is no longer connected, far to the right.
        assert_eq!(clamp(p(4000, 300), SIZE, AREA), p(1920 - 280, 300));
    }

    #[test]
    fn overlapping_the_taskbar_moves_above_it() {
        // The work area ends where the taskbar starts (1032); the widget reaches into it.
        assert_eq!(clamp(p(500, 900), SIZE, AREA), p(500, 1032 - 190));
    }

    #[test]
    fn larger_than_the_work_area_is_pinned() {
        let tiny = Rect {
            left: 10,
            top: 20,
            right: 200,
            bottom: 100,
        };
        assert_eq!(clamp(p(50, 50), SIZE, tiny), p(10, 20));
    }
}
