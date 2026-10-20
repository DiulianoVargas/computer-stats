//! Value text shown in the Panel (contracts/ui.md Rows; FR-011). 1 GB = 1024³ bytes, matching
//! Explorer and Task Manager.

use super::metric::Reason;

const GB: f64 = 1024.0 * 1024.0 * 1024.0;

/// `"23 %"`
pub fn percent(v: f32) -> String {
    format!("{} %", v.round() as i32)
}

/// `"54 °C"`
pub fn temperature(v: f32) -> String {
    format!("{} °C", v.round() as i32)
}

/// `"1240 RPM"`: no thousands separator (006 contracts/ui.md).
pub fn rpm(v: f32) -> String {
    format!("{} RPM", v.round() as i32)
}

/// `"18.4 / 31.9 GB"`
pub fn used_total(used: u64, total: u64) -> String {
    format!("{:.1} / {:.1} GB", used as f64 / GB, total as f64 / GB)
}

/// `"212 GB"`, or `"1.4 TB"` from 1000 GB up.
pub fn size(bytes: u64) -> String {
    let gb = bytes as f64 / GB;
    if gb.round() >= 1000.0 {
        format!("{:.1} TB", gb / 1024.0)
    } else {
        format!("{} GB", gb.round() as u64)
    }
}

/// `"212 GB free of 931 GB"`
pub fn drive_free(free: u64, total: u64) -> String {
    format!("{} free of {}", size(free), size(total))
}

/// `"C: System"`, or `"C:"` when the label is empty.
pub fn drive_label(letter: char, label: &str) -> String {
    let label = label.trim();
    if label.is_empty() {
        format!("{letter}:")
    } else {
        format!("{letter}: {label}")
    }
}

/// App memory: `"812 MB"` below 1000 MB, `"1.4 GB"` from 1000 MB (1 MB = 1024² bytes).
pub fn memory(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb.round() >= 1000.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{} MB", mb.round() as u64)
    }
}

/// `"Google Chrome (15)"`, or just the name for a single Process.
pub fn app_name_with_count(name: &str, processes: usize) -> String {
    if processes > 1 {
        format!("{name} ({processes})")
    } else {
        name.to_string()
    }
}

/// `"— no GPU"`
pub fn unavailable(reason: Reason) -> String {
    format!("— {}", reason.message())
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: u64 = 1024 * 1024 * 1024;

    #[test]
    fn percent_rounds_to_integer() {
        assert_eq!(percent(23.4), "23 %");
        assert_eq!(percent(23.5), "24 %");
        assert_eq!(percent(0.0), "0 %");
        assert_eq!(percent(100.0), "100 %");
    }

    #[test]
    fn temperature_rounds_to_integer() {
        assert_eq!(temperature(54.26), "54 °C");
        assert_eq!(temperature(89.5), "90 °C");
    }

    #[test]
    fn rpm_is_rounded_without_separator() {
        assert_eq!(rpm(1239.6), "1240 RPM");
        assert_eq!(rpm(0.0), "0 RPM");
    }

    #[test]
    fn used_total_one_decimal() {
        assert_eq!(
            used_total(18 * G + 4 * G / 10, 32 * G - G / 10),
            "18.4 / 31.9 GB"
        );
        assert_eq!(used_total(0, 12 * G), "0.0 / 12.0 GB");
    }

    #[test]
    fn size_switches_to_tb_at_1000_gb() {
        assert_eq!(size(212 * G), "212 GB");
        assert_eq!(size(999 * G), "999 GB");
        assert_eq!(size(1000 * G), "1.0 TB");
        assert_eq!(size(1843 * G), "1.8 TB");
        assert_eq!(size(0), "0 GB");
    }

    #[test]
    fn drive_texts() {
        assert_eq!(drive_free(212 * G, 931 * G), "212 GB free of 931 GB");
        assert_eq!(drive_label('C', "System"), "C: System");
        assert_eq!(drive_label('D', ""), "D:");
        assert_eq!(drive_label('E', "  "), "E:");
    }

    #[test]
    fn app_memory_switches_to_gb_at_1000_mb() {
        const MB: u64 = 1024 * 1024;
        assert_eq!(memory(0), "0 MB");
        assert_eq!(memory(812 * MB), "812 MB");
        assert_eq!(memory(999 * MB), "999 MB");
        assert_eq!(memory(1000 * MB), "1.0 GB");
        assert_eq!(memory(1434 * MB), "1.4 GB");
    }

    #[test]
    fn app_name_count() {
        assert_eq!(app_name_with_count("Chrome", 15), "Chrome (15)");
        assert_eq!(app_name_with_count("Notepad", 1), "Notepad");
    }

    #[test]
    fn unavailable_text() {
        assert_eq!(unavailable(Reason::NoGpu), "— no GPU");
        assert_eq!(unavailable(Reason::NotElevated), "— not elevated");
    }
}
