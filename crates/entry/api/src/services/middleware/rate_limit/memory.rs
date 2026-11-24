//! [`MemoryRateLimitBuckets`]: the per-replica windows the global limiter
//! charges when the users member provides no replica-shared ones.
//!
//! It counts the same per-user, per-scope windows in process memory, spread
//! over 64 independently locked shards so concurrent callers rarely
//! contend. Each shard holds one counter per `(user, scope)` — a counter from
//! an earlier window is reset when its caller is next charged — in a
//! least-recently-charged order bounded at 4096 callers. A new
//! caller in a full shard evicts that shard's least recently charged counter
//! only when it belongs to an earlier window; when every counter in the shard
//! is still in the current window the charge fails, and the limiter answers
//! `503`, so memory pressure never restores anyone's budget. No charge walks
//! more than its own shard's entry. Evictions and refusals are counted on
//! `rate_limit_memory_evictions_total` and `rate_limit_memory_refusals_total`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::hash::BuildHasher;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use lru::LruCache;
use systemprompt_identifiers::UserId;
use systemprompt_traits::{RateLimitBuckets, RepositoryError};

const SHARDS: usize = 64;
const SHARD_CAPACITY: NonZeroUsize = NonZeroUsize::new(4096).expect("non-zero");
const RATE_LIMIT_MEMORY_EVICTIONS_TOTAL: &str = "rate_limit_memory_evictions_total";
const RATE_LIMIT_MEMORY_REFUSALS_TOTAL: &str = "rate_limit_memory_refusals_total";

type Key = (UserId, String);

#[derive(Debug, Clone, Copy)]
struct Window {
    start: DateTime<Utc>,
    hits: i64,
}

#[derive(Debug)]
pub struct MemoryRateLimitBuckets {
    shards: Vec<Mutex<LruCache<Key, Window>>>,
    hasher: std::collections::hash_map::RandomState,
    evictions: AtomicU64,
    refusals: AtomicU64,
}

impl Default for MemoryRateLimitBuckets {
    fn default() -> Self {
        Self::with_shard_capacity(SHARD_CAPACITY)
    }
}

impl MemoryRateLimitBuckets {
    pub const SHARDS: usize = SHARDS;

    #[must_use]
    pub fn with_shard_capacity(capacity: NonZeroUsize) -> Self {
        Self {
            shards: (0..SHARDS)
                .map(|_| Mutex::new(LruCache::new(capacity)))
                .collect(),
            hasher: std::collections::hash_map::RandomState::new(),
            evictions: AtomicU64::new(0),
            refusals: AtomicU64::new(0),
        }
    }

    #[must_use]
    pub fn evictions(&self) -> u64 {
        self.evictions.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn refusals(&self) -> u64 {
        self.refusals.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn tracked(&self) -> usize {
        self.shards
            .iter()
            .map(|shard| shard.lock().unwrap_or_else(PoisonError::into_inner).len())
            .sum()
    }

    fn shard_of(&self, key: &Key) -> &Mutex<LruCache<Key, Window>> {
        let index = usize::try_from(self.hasher.hash_one(key) % SHARDS as u64).unwrap_or(0);
        &self.shards[index]
    }
}

#[async_trait]
impl RateLimitBuckets for MemoryRateLimitBuckets {
    async fn hit(
        &self,
        user_id: &UserId,
        scope: &str,
        window_start: DateTime<Utc>,
    ) -> Result<i64, RepositoryError> {
        let key = (user_id.clone(), scope.to_owned());
        let mut shard = self
            .shard_of(&key)
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(window) = shard.get_mut(&key) {
            if window.start < window_start {
                *window = Window {
                    start: window_start,
                    hits: 0,
                };
            }
            if window.start == window_start {
                window.hits = window.hits.saturating_add(1);
            }
            return Ok(window.hits);
        }
        if shard.len() >= shard.cap().get() {
            match shard.peek_lru() {
                Some((_, oldest)) if oldest.start < window_start => {
                    shard.pop_lru();
                    self.evictions.fetch_add(1, Ordering::Relaxed);
                    metrics::counter!(RATE_LIMIT_MEMORY_EVICTIONS_TOTAL).increment(1);
                },
                _ => {
                    drop(shard);
                    self.refusals.fetch_add(1, Ordering::Relaxed);
                    metrics::counter!(RATE_LIMIT_MEMORY_REFUSALS_TOTAL).increment(1);
                    return Err(RepositoryError::Conflict {
                        entity: "rate limit window",
                        key: scope.to_owned(),
                        reason: "every counter in the shard is in the current window".to_owned(),
                    });
                },
            }
        }
        shard.put(
            key,
            Window {
                start: window_start,
                hits: 1,
            },
        );
        drop(shard);
        Ok(1)
    }
}
