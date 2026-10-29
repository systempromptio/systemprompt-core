//! Process-wide cache for the [`ParentChainIndex`], revalidated against a
//! table fingerprint instead of rebuilt per decision.
//!
//! Loading the index costs three sequential round trips, which against a
//! cross-region database is 0.5–1 s on every authz decision. The cache
//! bounds staleness two ways. Within `recheck` of the last check it answers
//! from memory. Past that it spends one round trip on the fingerprint (the
//! committed `config_version`, which every rule and entity write advances in
//! commit order) and reloads only when it moved, so any committed rule or
//! entity change is visible within `recheck`. The fingerprint is read before
//! the index loads, so an index is never paired with a newer version than it
//! reflects. The `ttl` forces a reload regardless. A rule write on any
//! replica advances the [`CacheDomain::Authz`] epoch, which forces a reload
//! on the next decision regardless of either bound.
//!
//! Revalidation and reloads are single-flight: callers past `recheck` queue
//! on one load lock and re-read the slot once they hold it, so a stale slot
//! costs one fingerprint query and at most one index load per replica, while
//! readers of a fresh slot never wait on it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::{Duration, Instant};

use systemprompt_traits::{CacheDomain, CacheEpochs};
use tokio::sync::{Mutex, RwLock};

use super::{ChainSources, ParentChainIndex};
use crate::authz::error::AuthzResult;
use crate::authz::repository::{AccessControlRepository, ChainFingerprint};

const DEFAULT_TTL: Duration = Duration::from_secs(60);
const DEFAULT_RECHECK: Duration = Duration::from_secs(5);

#[derive(Debug)]
struct CachedIndex {
    index: Arc<ParentChainIndex>,
    fingerprint: ChainFingerprint,
    loaded_at: Instant,
    checked_at: Instant,
    epoch: u64,
}

#[derive(Debug)]
pub struct ChainIndexCache {
    slot: RwLock<Option<CachedIndex>>,
    load: Mutex<()>,
    ttl: Duration,
    recheck: Duration,
    epochs: CacheEpochs,
}

impl ChainIndexCache {
    #[must_use]
    pub fn new(ttl: Duration, recheck: Duration, epochs: CacheEpochs) -> Self {
        Self {
            slot: RwLock::new(None),
            load: Mutex::new(()),
            ttl,
            recheck,
            epochs,
        }
    }

    #[must_use]
    pub fn with_default_bounds(epochs: CacheEpochs) -> Self {
        Self::new(DEFAULT_TTL, DEFAULT_RECHECK, epochs)
    }

    pub async fn get(
        &self,
        repo: &AccessControlRepository,
        sources: Arc<ChainSources>,
    ) -> AuthzResult<Arc<ParentChainIndex>> {
        let epoch = self.epochs.current(CacheDomain::Authz);
        if let Some(index) = self.fresh(epoch).await {
            return Ok(index);
        }

        let _load = self.load.lock().await;
        if let Some(index) = self.fresh(epoch).await {
            return Ok(index);
        }

        let now = Instant::now();
        let fingerprint = repo.chain_fingerprint().await?;
        {
            let mut slot = self.slot.write().await;
            if let Some(cached) = slot.as_mut().filter(|cached| cached.epoch == epoch)
                && fingerprint == cached.fingerprint
                && now.duration_since(cached.loaded_at) < self.ttl
            {
                cached.checked_at = now;
                return Ok(Arc::clone(&cached.index));
            }
        }

        let index = Arc::new(ParentChainIndex::load(repo, sources).await?);
        *self.slot.write().await = Some(CachedIndex {
            index: Arc::clone(&index),
            fingerprint,
            loaded_at: now,
            checked_at: now,
            epoch,
        });
        Ok(index)
    }

    async fn fresh(&self, epoch: u64) -> Option<Arc<ParentChainIndex>> {
        let slot = self.slot.read().await;
        slot.as_ref()
            .filter(|cached| cached.epoch == epoch)
            .filter(|cached| cached.checked_at.elapsed() < self.recheck)
            .map(|cached| Arc::clone(&cached.index))
    }
}
