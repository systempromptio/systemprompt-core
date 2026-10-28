//! [`AgentRateLimiter`]: a per-user ceiling on agent edits per hour.
//!
//! The window is a sliding hour of edit timestamps per user, held in
//! process memory: each replica limits on its own, so the effective ceiling
//! across `n` replicas is up to `n` times the configured one. It bounds a
//! runaway agent, not a determined one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use systemprompt_identifiers::UserId;

use super::error::AgentRefusal;

const WINDOW: Duration = Duration::from_secs(3600);

#[derive(Debug)]
pub struct AgentRateLimiter {
    per_hour: u64,
    windows: Mutex<HashMap<UserId, VecDeque<Instant>>>,
}

impl AgentRateLimiter {
    #[must_use]
    pub fn new(per_hour: u64) -> Self {
        Self {
            per_hour,
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// Records `ops` agent edits for `user` at `now`, or refuses them all
    /// when they would pass the hourly ceiling.
    pub fn admit(&self, user: &UserId, ops: u64, now: Instant) -> Result<(), AgentRefusal> {
        let mut windows = self
            .windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let window = windows.entry(user.clone()).or_default();
        while window
            .front()
            .is_some_and(|at| now.saturating_duration_since(*at) >= WINDOW)
        {
            window.pop_front();
        }
        let ops = ops.max(1);
        if window.len() as u64 + ops > self.per_hour {
            let retry_after = window.front().map_or(WINDOW, |oldest| {
                WINDOW.saturating_sub(now.saturating_duration_since(*oldest))
            });
            return Err(AgentRefusal::RateLimited {
                limit: self.per_hour,
                retry_after_secs: retry_after.as_secs().max(1),
            });
        }
        window.extend(std::iter::repeat_n(
            now,
            usize::try_from(ops).unwrap_or(usize::MAX),
        ));
        Ok(())
    }
}
