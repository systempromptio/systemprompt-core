//! JWT JTI (token identifier) revocation. Logout writes a row; the request
//! authenticator consults this table through [`JtiRevocationChecker`] (the
//! [`TokenRevocations`] the kernel seeds) and its [`JtiRevocationCache`] on
//! every authenticated request. The `exp` column carries the JWT's original
//! expiry so cleanup can drop rows that are no longer load-bearing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OAuthRepository;
use crate::error::OauthResult;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use lru::LruCache;
use sqlx::PgPool;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AccessTokenId, UserId};
use systemprompt_models::auth::TokenRevocations;
use systemprompt_models::execution::context::ContextExtractionError;
use systemprompt_traits::{CacheDomain, CacheEpochView};

const NEGATIVE_TTL_SECONDS: u64 = 60;

const DEFAULT_CACHE_CAPACITY: usize = 5_000;

async fn jti_revoked(pool: &PgPool, jti: &AccessTokenId) -> OauthResult<bool> {
    let revoked = sqlx::query_scalar!(
        r#"SELECT EXISTS(
                 SELECT 1 FROM oauth_jti_revocations
                  WHERE jti = $1 AND exp > now()
               ) AS "exists!""#,
        jti.as_str(),
    )
    .fetch_one(pool)
    .await?;
    Ok(revoked)
}

impl OAuthRepository {
    pub async fn revoke_jti(
        &self,
        jti: &AccessTokenId,
        user_id: &UserId,
        exp: DateTime<Utc>,
    ) -> OauthResult<()> {
        sqlx::query!(
            "INSERT INTO oauth_jti_revocations (jti, user_id, exp)
             VALUES ($1, $2, $3)
             ON CONFLICT (jti) DO NOTHING",
            jti.as_str(),
            user_id.as_str(),
            exp,
        )
        .execute(self.write_pool_ref())
        .await?;
        Ok(())
    }

    pub async fn is_jti_revoked(&self, jti: &AccessTokenId) -> OauthResult<bool> {
        jti_revoked(self.write_pool_ref(), jti).await
    }

    pub async fn revoke_jtis_for_user(
        &self,
        user_id: &UserId,
        jtis: &[AccessTokenId],
        exp_floor: DateTime<Utc>,
    ) -> OauthResult<u64> {
        let mut inserted: u64 = 0;
        for jti in jtis {
            let result = sqlx::query!(
                "INSERT INTO oauth_jti_revocations (jti, user_id, exp)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (jti) DO NOTHING",
                jti.as_str(),
                user_id.as_str(),
                exp_floor,
            )
            .execute(self.write_pool_ref())
            .await?;
            inserted += result.rows_affected();
        }
        Ok(inserted)
    }
}

#[derive(Debug, Clone, Copy)]
enum CacheEntry {
    NotRevoked { inserted_at: Instant, epoch: u64 },
    Revoked,
}

/// In-memory LRU layered in front of `is_jti_revoked`.
///
/// Negative results
/// expire after 60s, or as soon as a revocation on any replica advances the
/// [`CacheDomain::JtiRevocation`] epoch; positive results are sticky (a
/// revoked jti cannot become un-revoked). A negative result is recorded under
/// the epoch read before its lookup, so a revocation notified while the lookup
/// was in flight leaves the entry already stale. A negative result is trusted
/// only while the invalidation relay is live: with no relay listening a
/// revocation on another replica cannot reach this cache, so every lookup
/// goes to the database.
pub struct JtiRevocationCache {
    cache: Mutex<LruCache<AccessTokenId, CacheEntry>>,
    epochs: CacheEpochView,
}

impl std::fmt::Debug for JtiRevocationCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JtiRevocationCache").finish_non_exhaustive()
    }
}

impl JtiRevocationCache {
    #[must_use]
    pub fn new(epochs: CacheEpochView) -> Self {
        Self::with_capacity(DEFAULT_CACHE_CAPACITY, epochs)
    }

    #[must_use]
    pub fn with_capacity(capacity: usize, epochs: CacheEpochView) -> Self {
        let cap = NonZeroUsize::new(capacity.max(1)).unwrap_or(NonZeroUsize::MIN);
        Self {
            cache: Mutex::new(LruCache::new(cap)),
            epochs,
        }
    }

    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.epochs.current(CacheDomain::JtiRevocation)
    }

    pub fn peek(&self, jti: &AccessTokenId) -> Option<bool> {
        let mut guard = self
            .cache
            .lock()
            .inspect_err(|e| tracing::warn!(error = %e, "JTI revocation cache lock is poisoned"))
            .ok()?;
        match guard.get(jti).copied()? {
            CacheEntry::Revoked => Some(true),
            CacheEntry::NotRevoked { inserted_at, epoch } => {
                if inserted_at.elapsed() < Duration::from_secs(NEGATIVE_TTL_SECONDS)
                    && epoch == self.epoch()
                    && self.epochs.relay_live()
                {
                    Some(false)
                } else {
                    guard.pop(jti);
                    None
                }
            },
        }
    }

    pub fn record(&self, jti: &AccessTokenId, revoked: bool, looked_up_at_epoch: u64) {
        if let Ok(mut guard) = self.cache.lock() {
            let entry = if revoked {
                CacheEntry::Revoked
            } else {
                CacheEntry::NotRevoked {
                    inserted_at: Instant::now(),
                    epoch: looked_up_at_epoch,
                }
            };
            guard.put(jti.clone(), entry);
        }
    }
}

/// The revocation check the request authenticator runs last: the cache, then
/// the table. Fails closed — a store error refuses the token rather than
/// admitting an unverifiable one.
#[derive(Clone)]
pub struct JtiRevocationChecker {
    pool: Arc<PgPool>,
    cache: Arc<JtiRevocationCache>,
}

impl std::fmt::Debug for JtiRevocationChecker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JtiRevocationChecker")
            .finish_non_exhaustive()
    }
}

impl JtiRevocationChecker {
    #[must_use]
    pub fn new(db: &DbPool, epochs: CacheEpochView) -> Self {
        Self {
            pool: db.write_pool(),
            cache: Arc::new(JtiRevocationCache::new(epochs)),
        }
    }
}

#[async_trait]
impl TokenRevocations for JtiRevocationChecker {
    async fn ensure_not_revoked(&self, jti: &AccessTokenId) -> Result<(), ContextExtractionError> {
        match self.cache.peek(jti) {
            Some(true) => return Err(ContextExtractionError::Revoked),
            Some(false) => return Ok(()),
            None => {},
        }

        let epoch = self.cache.epoch();
        let revoked = jti_revoked(&self.pool, jti).await.map_err(|e| {
            ContextExtractionError::DatabaseError {
                context: "JTI revocation lookup failed".to_owned(),
                source: e.into(),
            }
        })?;
        self.cache.record(jti, revoked, epoch);
        if revoked {
            Err(ContextExtractionError::Revoked)
        } else {
            Ok(())
        }
    }
}
