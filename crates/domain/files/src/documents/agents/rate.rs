//! [`AgentRateLimiter`]: ceilings on agent edits per hour, one per user and
//! agent and one for all of a user's agents together.
//!
//! The window is a sliding hour of edit batches, each one entry with its op
//! count, kept in process memory: each replica limits on its own, so the
//! effective ceiling across `n` replicas is up to `n` times the configured
//! one. It bounds a runaway agent, not a determined one. A batch is reserved
//! against both ceilings under one lock before it is written, and refunded
//! if it does not land, so concurrent writers cannot overshoot and a refused
//! or failed commit costs nothing. The map holds at most [`MAX_TRACKED`]
//! keys; past that, windows with nothing left in the hour go first, then
//! the least recently used.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use systemprompt_identifiers::UserId;

use super::error::AgentRefusal;

const WINDOW: Duration = Duration::from_secs(3600);

pub const MAX_TRACKED: usize = 10_000;

const ALL_AGENTS: &str = "";

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

    fn retry_after(&self, now: Instant) -> Duration {
        self.batches.front().map_or(WINDOW, |(at, _)| {
            WINDOW.saturating_sub(now.saturating_duration_since(*at))
        })
    }

    fn remove(&mut self, at: Instant, ops: u64) {
        if let Some(index) = self.batches.iter().position(|batch| *batch == (at, ops)) {
            self.batches.remove(index);
            self.total = self.total.saturating_sub(ops);
        }
    }
}

/// A batch counted against both ceilings; hand it back with
/// [`AgentRateLimiter::refund`] when the write does not land.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reservation {
    user: UserId,
    agent: String,
    ops: u64,
    at: Instant,
}

/// The agent-edit ceilings of one replica.
///
/// Windows live in process memory, so `n` replicas admit up to `n` times the
/// configured `agent_edits_per_hour` and `agent_edits_per_hour_per_user`
/// between them; size those limits per replica.
#[derive(Debug)]
pub struct AgentRateLimiter {
    per_agent: u64,
    per_user: u64,
    windows: Mutex<HashMap<Key, Window>>,
}

impl AgentRateLimiter {
    #[must_use]
    pub fn new(per_agent: u64, per_user: u64) -> Self {
        Self {
            per_agent,
            per_user: per_user.max(per_agent),
            windows: Mutex::new(HashMap::new()),
        }
    }

    pub fn reserve(
        &self,
        user: &UserId,
        agent: &str,
        ops: u64,
        now: Instant,
    ) -> Result<Reservation, AgentRefusal> {
        let ops = ops.max(1);
        let agent_key = (user.clone(), agent.to_owned());
        let user_key = (user.clone(), ALL_AGENTS.to_owned());
        let mut windows = self.lock();
        for (key, limit) in [(&agent_key, self.per_agent), (&user_key, self.per_user)] {
            if let Some(window) = windows.get_mut(key) {
                window.expire(now);
                if window.total.saturating_add(ops) > limit {
                    return Err(AgentRefusal::RateLimited {
                        limit,
                        retry_after_secs: window.retry_after(now).as_secs().max(1),
                    });
                }
            } else if ops > limit {
                return Err(AgentRefusal::RateLimited {
                    limit,
                    retry_after_secs: WINDOW.as_secs(),
                });
            }
        }
        for key in [agent_key, user_key] {
            if !windows.contains_key(&key) && windows.len() >= MAX_TRACKED {
                evict(&mut windows, now);
            }
            let window = windows.entry(key).or_default();
            window.batches.push_back((now, ops));
            window.total = window.total.saturating_add(ops);
            window.used = Some(now);
        }
        drop(windows);
        Ok(Reservation {
            user: user.clone(),
            agent: agent.to_owned(),
            ops,
            at: now,
        })
    }

    pub fn refund(&self, reservation: &Reservation) {
        let mut windows = self.lock();
        for agent in [reservation.agent.as_str(), ALL_AGENTS] {
            if let Some(window) = windows.get_mut(&(reservation.user.clone(), agent.to_owned())) {
                window.remove(reservation.at, reservation.ops);
            }
        }
    }

    #[must_use]
    pub fn tracked(&self) -> usize {
        self.lock().len()
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
