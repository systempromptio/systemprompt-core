//! Reconnect pacing for the bridge stream: exponential, jittered by ±20%,
//! and reset only once a connection has stayed open for [`HEALTHY_AFTER`].
//!
//! A gateway that accepts the stream and closes it at once would otherwise be
//! redialled every [`RETRY_MIN`] by every bridge in lockstep.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

pub const RETRY_MIN: Duration = Duration::from_secs(1);
pub const RETRY_MAX: Duration = Duration::from_secs(60);
pub const HEALTHY_AFTER: Duration = Duration::from_secs(30);
pub const JITTER: f64 = 0.2;

/// The delay before the next stream attempt, grown on every short-lived
/// connection whether it failed or closed cleanly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectBackoff {
    base: Duration,
}

impl Default for ReconnectBackoff {
    fn default() -> Self {
        Self { base: RETRY_MIN }
    }
}

impl ReconnectBackoff {
    #[must_use]
    pub const fn base(&self) -> Duration {
        self.base
    }

    pub const fn reset(&mut self) {
        self.base = RETRY_MIN;
    }

    pub fn next_delay(&mut self, open_for: Duration, unit: f64) -> Duration {
        if open_for >= HEALTHY_AFTER {
            self.base = RETRY_MIN;
        }
        let delay = jittered(self.base, unit);
        self.base = self.base.saturating_mul(2).min(RETRY_MAX);
        delay
    }
}

#[must_use]
pub fn jittered(base: Duration, unit: f64) -> Duration {
    let factor = JITTER.mul_add(unit.clamp(0.0, 1.0).mul_add(2.0, -1.0), 1.0);
    base.mul_f64(factor)
}
