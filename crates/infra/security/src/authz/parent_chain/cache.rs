//! Process-wide cache for the [`ParentChainIndex`], revalidated against a
//! table fingerprint instead of rebuilt per decision.
//!
//! Loading the index costs three sequential round trips, which against a
//! cross-region database is 0.5–1 s on every authz decision. The cache
//! bounds staleness two ways. Within `recheck` of the last check it answers
//! from memory. Past that it spends one round trip on the fingerprint (row
//! counts plus `MAX(updated_at)` of both tables) and reloads only when it
//! moved, so a rule change is visible within `recheck` of its `updated_at`
//! bump and a delete moves the count. The `ttl` forces a reload regardless,
//! bounding any change the fingerprint cannot see.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

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
}

// Why: a per-decision rebuild is the cross-region cost the module head
// describes; the fingerprint and TTL are the two staleness bounds.
#[derive(Debug)]
pub struct ChainIndexCache {
    slot: RwLock<Option<CachedIndex>>,
    ttl: Duration,
    recheck: Duration,
}

impl Default for ChainIndexCache {
    fn default() -> Self {
        Self::new(DEFAULT_TTL, DEFAULT_RECHECK)
    }
}

impl ChainIndexCache {
    #[must_use]
    pub fn new(ttl: Duration, recheck: Duration) -> Self {
        Self {
            slot: RwLock::new(None),
            ttl,
            recheck,
        }
    }

    pub async fn get(
        &self,
        repo: &AccessControlRepository,
        sources: Arc<ChainSources>,
    ) -> AuthzResult<Arc<ParentChainIndex>> {
        let now = Instant::now();
        let fresh = {
            let slot = self.slot.read().await;
            slot.as_ref()
                .filter(|cached| now.duration_since(cached.checked_at) < self.recheck)
                .map(|cached| Arc::clone(&cached.index))
        };
        if let Some(index) = fresh {
            return Ok(index);
        }

        {
            let mut slot = self.slot.write().await;
            if let Some(cached) = slot.as_mut() {
                if now.duration_since(cached.checked_at) < self.recheck {
                    return Ok(Arc::clone(&cached.index));
                }
                // Why: a fingerprint fault falls through to a full reload rather
                // than serving the cached index, so a database fault never keeps
                // a stale index alive silently.
                if let Ok(fingerprint) = repo.chain_fingerprint().await
                    && fingerprint == cached.fingerprint
                    && now.duration_since(cached.loaded_at) < self.ttl
                {
                    cached.checked_at = now;
                    return Ok(Arc::clone(&cached.index));
                }
            }
        }

        let fingerprint = repo.chain_fingerprint().await?;
        let index = Arc::new(ParentChainIndex::load(repo, sources).await?);
        *self.slot.write().await = Some(CachedIndex {
            index: Arc::clone(&index),
            fingerprint,
            loaded_at: now,
            checked_at: now,
        });
        Ok(index)
    }
}
