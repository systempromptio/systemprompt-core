//! Interval-based suppression for repeated log emissions.
//!
//! [`LogThrottle`] gates a hot-path warning down to at most one emission per
//! interval; under concurrency at most one caller wins each interval.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct LogThrottle {
    interval_secs: u64,
    last_emit_epoch_secs: AtomicU64,
}

impl LogThrottle {
    #[must_use]
    pub const fn new(interval_secs: u64) -> Self {
        Self {
            interval_secs,
            last_emit_epoch_secs: AtomicU64::new(0),
        }
    }

    #[must_use]
    pub fn allow(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        self.allow_at(now)
    }

    #[must_use]
    pub fn allow_at(&self, now_epoch_secs: u64) -> bool {
        let last = self.last_emit_epoch_secs.load(Ordering::Acquire);
        if last != 0 && now_epoch_secs.saturating_sub(last) < self.interval_secs {
            return false;
        }
        self.last_emit_epoch_secs
            .compare_exchange(last, now_epoch_secs, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }
}
