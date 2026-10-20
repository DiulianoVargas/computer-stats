//! End App's in-place, two-click confirmation (003 FR-012, FR-014; data-model.md).
//!
//! ```text
//! Idle ──click(k)──▶ Armed{k, until = now + 3 s}
//! Armed{k} ──click(k), now < until──▶ Idle + End(k)
//! Armed{k} ──click(j ≠ k)──▶ Armed{j, now + 3 s}
//! Armed{k} ──now ≥ until | cancel()──▶ Idle
//! ```

use std::time::{Duration, Instant};

use super::apps::AppKey;

pub const ARM_TIME: Duration = Duration::from_secs(3);

#[derive(Debug, Default)]
pub struct EndConfirm {
    armed: Option<(AppKey, Instant)>,
}

impl EndConfirm {
    /// A click on `key`'s End button. Returns the App to End, which is only ever the armed one.
    pub fn click(&mut self, key: &str, now: Instant) -> Option<AppKey> {
        self.expire(now);
        match self.armed.take() {
            Some((armed, _)) if armed == key => Some(armed),
            _ => {
                self.armed = Some((key.to_string(), now + ARM_TIME));
                None
            }
        }
    }

    /// Disarms once the arming time is over.
    pub fn expire(&mut self, now: Instant) {
        if self.armed.as_ref().is_some_and(|(_, until)| now >= *until) {
            self.armed = None;
        }
    }

    pub fn cancel(&mut self) {
        self.armed = None;
    }

    pub fn armed(&self) -> Option<&str> {
        self.armed.as_ref().map(|(key, _)| key.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_click_only_arms() {
        let mut c = EndConfirm::default();
        let t = Instant::now();
        assert_eq!(c.click("a", t), None);
        assert_eq!(c.armed(), Some("a"));
    }

    #[test]
    fn second_click_in_time_ends_and_disarms() {
        let mut c = EndConfirm::default();
        let t = Instant::now();
        c.click("a", t);
        assert_eq!(
            c.click("a", t + Duration::from_millis(2999)),
            Some("a".to_string())
        );
        assert_eq!(c.armed(), None);
    }

    #[test]
    fn second_click_after_three_seconds_rearms_instead() {
        let mut c = EndConfirm::default();
        let t = Instant::now();
        c.click("a", t);
        assert_eq!(c.click("a", t + ARM_TIME), None);
        assert_eq!(c.armed(), Some("a"));
    }

    #[test]
    fn clicking_another_app_moves_the_arming() {
        let mut c = EndConfirm::default();
        let t = Instant::now();
        c.click("a", t);
        assert_eq!(c.click("b", t), None);
        assert_eq!(c.armed(), Some("b"));
        // "a" is no longer armed: clicking it arms it, it does not End it.
        assert_eq!(c.click("a", t), None);
    }

    #[test]
    fn expire_and_cancel() {
        let mut c = EndConfirm::default();
        let t = Instant::now();
        c.click("a", t);
        c.expire(t + Duration::from_secs(1));
        assert_eq!(c.armed(), Some("a"));
        c.expire(t + ARM_TIME);
        assert_eq!(c.armed(), None);

        c.click("a", t);
        c.cancel();
        assert_eq!(c.click("a", t), None);
    }

    #[test]
    fn never_ends_an_app_that_was_not_armed() {
        // SC-005: any interleaving returns only the key armed by the previous click.
        let keys = ["a", "b", "c"];
        let mut c = EndConfirm::default();
        let t = Instant::now();
        let mut last: Option<&str> = None;
        for i in 0..200usize {
            let key = keys[(i * 7 + i / 3) % 3];
            if let Some(ended) = c.click(key, t) {
                assert_eq!(Some(ended.as_str()), last);
                assert_eq!(ended, key);
                last = None;
            } else {
                last = Some(key);
            }
        }
    }
}
