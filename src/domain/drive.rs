//! A Drive: a fixed internal volume with a drive letter (CONTEXT.md). Identity is the letter.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drive {
    pub letter: char,
    /// May be empty.
    pub label: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

impl Drive {
    /// Disk Space used, 0–100 (0 for an empty or zero-size volume).
    pub fn used_pct(&self) -> f32 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        let used = self.total_bytes.saturating_sub(self.free_bytes);
        (used as f64 / self.total_bytes as f64 * 100.0) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive(total: u64, free: u64) -> Drive {
        Drive {
            letter: 'C',
            label: String::new(),
            total_bytes: total,
            free_bytes: free,
        }
    }

    #[test]
    fn used_pct() {
        assert_eq!(drive(100, 25).used_pct(), 75.0);
        assert_eq!(drive(100, 100).used_pct(), 0.0);
        assert_eq!(drive(100, 0).used_pct(), 100.0);
        assert_eq!(drive(0, 0).used_pct(), 0.0);
        // free > total (inconsistent read) saturates to 0 % used
        assert_eq!(drive(100, 150).used_pct(), 0.0);
    }
}
