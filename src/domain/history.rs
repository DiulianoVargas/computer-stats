//! Sparkline history: the last 60 one-second samples of a Metric, with gaps (data-model.md).
//! Fixed capacity, so memory never grows (Constitution VII).

pub const CAPACITY: usize = 60;

#[derive(Debug, Clone, PartialEq)]
pub struct History {
    samples: [Option<f32>; CAPACITY],
    /// Index of the next write.
    next: usize,
    /// Samples pushed so far, up to `CAPACITY`.
    len: usize,
}

impl Default for History {
    fn default() -> Self {
        History {
            samples: [None; CAPACITY],
            next: 0,
            len: 0,
        }
    }
}

impl History {
    /// `Some(value)` for a reading, `None` for a gap (unavailable or asleep).
    pub fn push(&mut self, sample: Option<f32>) {
        self.samples[self.next] = sample;
        self.next = (self.next + 1) % CAPACITY;
        self.len = (self.len + 1).min(CAPACITY);
    }

    /// `n` gap seconds (capped at a full history).
    pub fn push_gaps(&mut self, n: usize) {
        for _ in 0..n.min(CAPACITY) {
            self.push(None);
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Oldest → newest, only the samples pushed so far (nothing fabricated before the first tick).
    pub fn iter(&self) -> impl Iterator<Item = Option<f32>> + '_ {
        let start = (self.next + CAPACITY - self.len) % CAPACITY;
        (0..self.len).map(move |i| self.samples[(start + i) % CAPACITY])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_fill_has_no_fabricated_values() {
        let mut h = History::default();
        assert!(h.is_empty());
        h.push(Some(1.0));
        h.push(None);
        h.push(Some(3.0));
        assert_eq!(h.len(), 3);
        assert_eq!(h.iter().collect::<Vec<_>>(), [Some(1.0), None, Some(3.0)]);
    }

    #[test]
    fn wraps_keeping_the_newest_60() {
        let mut h = History::default();
        for i in 0..100 {
            h.push(Some(i as f32));
        }
        assert_eq!(h.len(), CAPACITY);
        let all: Vec<_> = h.iter().collect();
        assert_eq!(all.first(), Some(&Some(40.0)));
        assert_eq!(all.last(), Some(&Some(99.0)));
    }

    #[test]
    fn gaps_are_capped() {
        let mut h = History::default();
        h.push(Some(5.0));
        h.push_gaps(1000);
        assert_eq!(h.len(), CAPACITY);
        assert!(h.iter().all(|s| s.is_none()));
    }
}
