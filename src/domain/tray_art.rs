//! Tray Icon pixels drawn in code (research R3): three ascending bars.

/// The taskbar's theme; the glyph contrasts with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayTheme {
    Light,
    Dark,
}

const WHITE: u32 = 0xFFFF_FFFF;
const NEAR_BLACK: u32 = 0xFF1C_1C1C;

/// Premultiplied ARGB pixels, row-major, top row first, `size_px × size_px`.
pub fn render(size_px: u32, theme: TrayTheme) -> Vec<u32> {
    let n = size_px as usize;
    let mut pixels = vec![0u32; n * n];
    let glyph = match theme {
        TrayTheme::Dark => WHITE,
        TrayTheme::Light => NEAR_BLACK,
    };

    // Three bars on a grid of 7 columns: bar, gap, bar, gap, bar, with 1-column margins.
    let s = size_px as f32;
    let col = s / 7.0;
    let bottom = s * 0.875;
    let heights = [0.35, 0.55, 0.75];
    for (i, h) in heights.iter().enumerate() {
        let x0 = (col * (1 + 2 * i) as f32).round() as usize;
        let x1 = ((col * (2 + 2 * i) as f32).round() as usize).max(x0 + 1);
        let y0 = (bottom - s * h).round() as usize;
        let y1 = bottom.round() as usize;
        fill(&mut pixels, n, x0..x1, y0..y1, glyph);
    }
    pixels
}

fn fill(
    pixels: &mut [u32],
    n: usize,
    xs: std::ops::Range<usize>,
    ys: std::ops::Range<usize>,
    color: u32,
) {
    for y in ys.start.min(n)..ys.end.min(n) {
        for x in xs.start.min(n)..xs.end.min(n) {
            pixels[y * n + x] = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_and_transparent_corners() {
        for size in [16, 20, 24, 32] {
            let p = render(size, TrayTheme::Dark);
            let n = size as usize;
            assert_eq!(p.len(), n * n);
            for i in [0, n - 1, n * (n - 1), n * n - 1] {
                assert_eq!(p[i], 0, "size {size}, corner {i}");
            }
        }
    }

    #[test]
    fn glyph_color_follows_theme() {
        for size in [16, 24, 32] {
            let dark = render(size, TrayTheme::Dark);
            let light = render(size, TrayTheme::Light);
            assert!(dark.contains(&WHITE), "size {size}");
            assert!(light.contains(&NEAR_BLACK), "size {size}");
            assert!(!light.contains(&WHITE));
        }
    }

    #[test]
    fn bars_ascend() {
        let n = 32usize;
        let p = render(32, TrayTheme::Dark);
        let height = |x: usize| (0..n).filter(|&y| p[y * n + x] != 0).count();
        let col = 32.0 / 7.0;
        let centers = [1.5, 3.5, 5.5].map(|c: f32| (col * c) as usize);
        assert!(height(centers[0]) < height(centers[1]));
        assert!(height(centers[1]) < height(centers[2]));
    }
}
