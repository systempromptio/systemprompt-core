//! The in-process keyed limiter behind both per-replica throttles.
//!
//! Each key holds one GCRA theoretical-arrival time. A key whose arrival time
//! has passed is exactly as if it had never been seen, so dropping it changes
//! no decision: [`KeyedLimiter::evict_idle`] removes those keys, and the
//! sweeper spawned by [`KeyedLimiter::spawn_sweeper`] runs it on an interval
//! for as long as the limiter is alive.
//!
//! The map is also capped, with one of its slots reserved for the overflow
//! bucket, so it never holds more than `cap` keys. A key that is not yet
//! tracked when the map is full first triggers an idle sweep; if the map is
//! still full, the newcomer is charged against one shared overflow bucket
//! instead of getting its own. Tracked clients keep their own budgets, so a
//! flood of fresh keys cannot lock them out; it can only exhaust the overflow
//! budget it shares.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::{Duration, Instant};

use systemprompt_traits::OwnedTask;

pub const MAX_TRACKED_KEYS: usize = 100_000;
pub const SWEEP_INTERVAL: Duration = Duration::from_secs(60);
const OVERFLOW_KEY: &str = "overflow";

/// One limiter's per-key state, bounded by eviction and by a key cap.
#[derive(Debug)]
pub struct KeyedLimiter {
    emission: Duration,
    tolerance: Duration,
    cap: usize,
    arrivals: Mutex<HashMap<String, Instant>>,
}

impl KeyedLimiter {
    #[must_use]
    pub fn new(emission: Duration, burst: u32, cap: usize) -> Self {
        Self {
            emission,
            tolerance: emission.saturating_mul(burst.max(1).saturating_sub(1)),
            cap: cap.max(2),
            arrivals: Mutex::new(HashMap::new()),
        }
    }

    pub fn check(&self, key: &str, now: Instant) -> Result<(), Duration> {
        let mut arrivals = self.arrivals.lock().unwrap_or_else(PoisonError::into_inner);
        let mut own = arrivals.contains_key(key) || self.has_client_slot(&arrivals);
        if !own {
            arrivals.retain(|_, tat| *tat > now);
            own = self.has_client_slot(&arrivals);
        }
        let key = if own { key } else { OVERFLOW_KEY };
        let tat = arrivals.get(key).copied().unwrap_or(now).max(now);
        let wait = tat.saturating_duration_since(now);
        if wait > self.tolerance {
            return Err(wait.saturating_sub(self.tolerance));
        }
        arrivals.insert(key.to_owned(), tat + self.emission);
        drop(arrivals);
        Ok(())
    }

    fn has_client_slot(&self, arrivals: &HashMap<String, Instant>) -> bool {
        let clients = arrivals.len() - usize::from(arrivals.contains_key(OVERFLOW_KEY));
        clients < self.cap - 1
    }

    pub fn evict_idle(&self, now: Instant) {
        self.arrivals
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|_, tat| *tat > now);
    }

    #[must_use]
    pub fn tracked_keys(&self) -> usize {
        self.arrivals
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    pub fn spawn_sweeper(self: &Arc<Self>, interval: Duration) -> OwnedTask<()> {
        let limiter: Weak<Self> = Arc::downgrade(self);
        OwnedTask::spawn("rate_limit_sweeper", async move {
            let mut ticks = tokio::time::interval(interval);
            ticks.tick().await;
            loop {
                ticks.tick().await;
                let Some(limiter) = limiter.upgrade() else {
                    return;
                };
                limiter.evict_idle(Instant::now());
            }
        })
    }
}
