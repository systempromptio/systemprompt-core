//! [`AgentRateLimiter`]: a ceiling on agent edits per hour, per user and
//! agent.
//!
//! The window is a sliding hour of edit batches, each one entry with its op
//! count, kept per `(user, agent)` in process memory: each replica limits on
//! its own, so the effective ceiling across `n` replicas is up to `n` times
//! the configured one. It bounds a runaway agent, not a determined one. A
//! batch is checked before it is written and recorded only once it lands, so
//! a refused or failed commit costs nothing. The map holds at most
//! [`MAX_TRACKED`] keys; past that, windows with nothing left in the hour
//! go first, then the least recently used.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use systemprompt_identifiers::UserId;

use super::error::AgentRefusal;

const WINDOW: Duration = Duration::from_secs(3600);

/// Keys kept at most; the oldest are evicted past it.
pub const MAX_TRACKED: usize = 10_000;

type Key = (UserId, String);

#[derive(Debug, Default)]
struct Window {
    batches: VecDeque<(Instant, u64)>,
    total: u64,
    used: Option<Instant>,
}

impl Window {
    fn expire(&mut self, now: Instant) {
        while let Some((at, ops)) = self.batches.front().copied() {
            if now.saturating_duration_since(at) < WINDOW {
                break;
            }
            self.batches.pop_front();
            self.total = self.total.saturating_sub(ops);
        }
    }
}

#[derive(Debug)]
pub struct AgentRateLimiter {
    per_hour: u64,
    windows: Mutex<HashMap<Key, Window>>,
}

impl AgentRateLimiter {
    #[must_use]
    pub fn new(per_hour: u64) -> Self {
        Self {
            per_hour,
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// Refuses a batch of `ops` edits by `agent` for `user` that would pass
    /// the hourly ceiling; records nothing.
    pub fn check(
        &self,
        user: &UserId,
        agent: &str,
        ops: u64,
        now: Instant,
    ) -> Result<(), AgentRefusal> {
        let mut windows = self.lock();
        let Some(window) = windows.get_mut(&(user.clone(), agent.to_owned())) else {
            return self.fits(0, ops, None);
        };
        window.expire(now);
        let oldest = window.batches.front().map(|(at, _)| *at);
        let retry = oldest.map(|at| WINDOW.saturating_sub(now.saturating_duration_since(at)));
        self.fits(window.total, ops, retry)
    }

    /// Records a landed batch of `ops` edits by `agent` for `user`.
    pub fn record(&self, user: &UserId, agent: &str, ops: u64, now: Instant) {
        let mut windows = self.lock();
        let key = (user.clone(), agent.to_owned());
        if !windows.contains_key(&key) && windows.len() >= MAX_TRACKED {
            evict(&mut windows, now);
        }
        let window = windows.entry(key).or_default();
        window.expire(now);
        window.batches.push_back((now, ops.max(1)));
        window.total = window.total.saturating_add(ops.max(1));
        window.used = Some(now);
    }

    /// [`Self::check`] then [`Self::record`], for a caller with nothing to
    /// fail between them.
    pub fn admit(
        &self,
        user: &UserId,
        agent: &str,
        ops: u64,
        now: Instant,
    ) -> Result<(), AgentRefusal> {
        self.check(user, agent, ops, now)?;
        self.record(user, agent, ops, now);
        Ok(())
    }

    /// How many `(user, agent)` windows are held.
    #[must_use]
    pub fn tracked(&self) -> usize {
        self.lock().len()
    }

    fn fits(&self, total: u64, ops: u64, retry: Option<Duration>) -> Result<(), AgentRefusal> {
        if total.saturating_add(ops.max(1)) <= self.per_hour {
            return Ok(());
        }
        Err(AgentRefusal::RateLimited {
            limit: self.per_hour,
            retry_after_secs: retry.unwrap_or(WINDOW).as_secs().max(1),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Key, Window>> {
        self.windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn evict(windows: &mut HashMap<Key, Window>, now: Instant) {
    windows.retain(|_, window| {
        window.expire(now);
        window.total > 0
    });
    if windows.len() < MAX_TRACKED {
        return;
    }
    let oldest = windows
        .iter()
        .min_by_key(|(_, window)| window.used)
        .map(|(key, _)| key.clone());
    if let Some(key) = oldest {
        windows.remove(&key);
    }
}
