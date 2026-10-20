//! Writes `assets/app.ico` from `domain::tray_art::app_icon` (16–256 px, 32-bpp BMP entries).
//! Run after changing the icon: `cargo run --example make_icon`.

use std::io::Write;

use computer_stats::domain::tray_art::app_icon;

const SIZES: [u32; 7] = [16, 20, 24, 32, 48, 64, 256];

fn main() -> std::io::Result<()> {
    let images: Vec<Vec<u8>> = SIZES.iter().map(|&s| bmp_entry(s)).collect();

    let mut ico = Vec::new();
    // ICONDIR
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&(SIZES.len() as u16).to_le_bytes());
    // ICONDIRENTRY × n
    let mut offset = 6 + 16 * SIZES.len() as u32;
    for (size, image) in SIZES.iter().zip(&images) {
        let dim = if *size >= 256 { 0 } else { *size as u8 };
        ico.extend_from_slice(&[dim, dim, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes()); // planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bit count
        ico.extend_from_slice(&(image.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += image.len() as u32;
    }
    for image in &images {
        ico.extend_from_slice(image);
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/app.ico");
    std::fs::File::create(&path)?.write_all(&ico)?;
    println!("wrote {} ({} bytes)", path.display(), ico.len());
    Ok(())
}

/// BITMAPINFOHEADER + bottom-up BGRA pixels (straight alpha) + 1-bpp AND mask.
fn bmp_entry(size: u32) -> Vec<u8> {
    let pixels = app_icon(size);
    let n = size as usize;
    let mask_row = n.div_ceil(32) * 4;
    let mut out = Vec::new();
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(size as i32).to_le_bytes());
    out.extend_from_slice(&(2 * size as i32).to_le_bytes()); // colour + mask
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&((n * n * 4 + mask_row * n) as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 16]); // resolution, colours used/important
    for y in (0..n).rev() {
        for x in 0..n {
            let p = pixels[y * n + x];
            let a = (p >> 24) & 0xFF;
            // Premultiplied → straight.
            let un = |c: u32| {
                if a == 0 {
                    0
                } else {
                    ((c * 255 + a / 2) / a).min(255) as u8
                }
            };
            out.extend_from_slice(&[
                un(p & 0xFF),
                un((p >> 8) & 0xFF),
                un((p >> 16) & 0xFF),
                a as u8,
            ]);
        }
    }
    for y in (0..n).rev() {
        let mut row = vec![0u8; mask_row];
        for x in 0..n {
            if pixels[y * n + x] >> 24 == 0 {
                row[x / 8] |= 0x80 >> (x % 8);
            }
        }
        out.extend_from_slice(&row);
    }
    out
}
