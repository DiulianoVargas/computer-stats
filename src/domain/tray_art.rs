//! Tray Icon pixels drawn in code (research R3): three ascending bars, plus the Status Dot.

use super::warning::WarningLevel;

/// The taskbar's theme; the glyph contrasts with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayTheme {
    Light,
    Dark,
}

const WHITE: u32 = 0xFFFF_FFFF;
const NEAR_BLACK: u32 = 0xFF1C_1C1C;
pub const AMBER: u32 = 0xFFF5_A623;
pub const RED: u32 = 0xFFE5_484D;
/// Approximate taskbar backgrounds, for the ring that separates the dot from the glyph.
const DARK_TASKBAR: u32 = 0xFF20_2020;
const LIGHT_TASKBAR: u32 = 0xFFEE_EEEE;

/// Premultiplied ARGB pixels, row-major, top row first, `size_px × size_px`. A Status Dot is drawn
/// in the lower right when `dot` is above Normal (amber for Warning, red for Critical).
pub fn render(size_px: u32, theme: TrayTheme, dot: WarningLevel) -> Vec<u32> {
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

    let dot_color = match dot {
        WarningLevel::Normal => return pixels,
        WarningLevel::Warning => AMBER,
        WarningLevel::Critical => RED,
    };
    let ring_color = match theme {
        TrayTheme::Dark => DARK_TASKBAR,
        TrayTheme::Light => LIGHT_TASKBAR,
    };
    let radius = s * 0.26;
    let ring = radius + (s / 16.0).max(1.0);
    let center = s - radius - 0.5;
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 + 0.5 - center, y as f32 + 0.5 - center);
            let d = (dx * dx + dy * dy).sqrt();
            if d <= radius {
                pixels[y * n + x] = dot_color;
            } else if d <= ring {
                pixels[y * n + x] = ring_color;
            }
        }
    }
    pixels
}

/// The application icon (Explorer, Start menu, installer): the three bars in white on a blue
/// rounded square, so it reads on light and dark backgrounds. Premultiplied ARGB, top row first;
/// edges are anti-aliased with 4 × 4 supersampling.
pub fn app_icon(size_px: u32) -> Vec<u32> {
    const BLUE: [f32; 3] = [0x0F as f32, 0x6C as f32, 0xBD as f32];
    const SS: u32 = 4;
    let n = size_px as usize;
    let s = size_px as f32;
    let radius = s * 0.22;
    let inset = s * 0.16;
    let glyph = render(size_px, TrayTheme::Dark, WarningLevel::Normal);
    let mut out = vec![0u32; n * n];
    for y in 0..n {
        for x in 0..n {
            // Coverage of the rounded square at this pixel.
            let mut covered = 0u32;
            for sy in 0..SS {
                for sx in 0..SS {
                    let px = x as f32 + (sx as f32 + 0.5) / SS as f32;
                    let py = y as f32 + (sy as f32 + 0.5) / SS as f32;
                    if in_rounded_square(px, py, s, radius) {
                        covered += 1;
                    }
                }
            }
            if covered == 0 {
                continue;
            }
            let alpha = covered as f32 / (SS * SS) as f32;
            // The glyph, shrunk into the square's inner area (nearest sample).
            let gx = ((x as f32 - inset) / (s - 2.0 * inset) * s) as isize;
            let gy = ((y as f32 - inset) / (s - 2.0 * inset) * s) as isize;
            let on_glyph = (0..n as isize).contains(&gx)
                && (0..n as isize).contains(&gy)
                && glyph[gy as usize * n + gx as usize] != 0;
            let rgb = if on_glyph { [255.0; 3] } else { BLUE };
            out[y * n + x] = premultiplied(rgb, alpha);
        }
    }
    out
}

fn in_rounded_square(x: f32, y: f32, s: f32, r: f32) -> bool {
    let cx = x.clamp(r, s - r);
    let cy = y.clamp(r, s - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

fn premultiplied(rgb: [f32; 3], alpha: f32) -> u32 {
    let a = (alpha * 255.0).round() as u32;
    let c = |v: f32| (v * alpha).round() as u32;
    (a << 24) | (c(rgb[0]) << 16) | (c(rgb[1]) << 8) | c(rgb[2])
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
    use WarningLevel::*;

    #[test]
    fn size_and_transparent_corners() {
        for size in [16, 20, 24, 32] {
            let p = render(size, TrayTheme::Dark, Normal);
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
            let dark = render(size, TrayTheme::Dark, Normal);
            let light = render(size, TrayTheme::Light, Normal);
            assert!(dark.contains(&WHITE), "size {size}");
            assert!(light.contains(&NEAR_BLACK), "size {size}");
            assert!(!light.contains(&WHITE));
        }
    }

    #[test]
    fn bars_ascend() {
        let n = 32usize;
        let p = render(32, TrayTheme::Dark, Normal);
        let height = |x: usize| (0..n).filter(|&y| p[y * n + x] != 0).count();
        let col = 32.0 / 7.0;
        let centers = [1.5, 3.5, 5.5].map(|c: f32| (col * c) as usize);
        assert!(height(centers[0]) < height(centers[1]));
        assert!(height(centers[1]) < height(centers[2]));
    }

    #[test]
    fn no_dot_when_normal() {
        for theme in [TrayTheme::Dark, TrayTheme::Light] {
            let p = render(32, theme, Normal);
            assert!(!p.contains(&AMBER) && !p.contains(&RED));
        }
    }

    #[test]
    fn dot_color_at_lower_right() {
        for size in [16u32, 24, 32] {
            let n = size as usize;
            let center = |p: &[u32]| {
                let c = (size as f32 - size as f32 * 0.26 - 0.5) as usize;
                p[c * n + c]
            };
            assert_eq!(center(&render(size, TrayTheme::Dark, Warning)), AMBER);
            assert_eq!(center(&render(size, TrayTheme::Light, Critical)), RED);
            // Top-left stays transparent.
            assert_eq!(render(size, TrayTheme::Dark, Critical)[0], 0);
        }
    }

    #[test]
    fn app_icon_is_a_blue_rounded_square_with_white_bars() {
        for size in [16u32, 32, 48, 256] {
            let n = size as usize;
            let p = app_icon(size);
            assert_eq!(p.len(), n * n);
            // Rounded corners stay transparent; the centre-left edge is opaque blue.
            assert_eq!(p[0], 0, "size {size}");
            assert_eq!(p[(n / 2) * n + 1], 0xFF0F_6CBD, "size {size}");
            // Some white glyph pixels.
            assert!(p.contains(&0xFFFF_FFFF), "size {size}");
        }
    }
}
