//! JWT JTI (token identifier) revocation. Logout writes a row; the JTI tower
//! middleware consults this table (through [`JtiRevocationCache`]) on every
//! authenticated request. The `exp` column carries the JWT's original expiry
//! so cleanup can drop rows that are no longer load-bearing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OAuthRepository;
use crate::error::OauthResult;
use chrono::{DateTime, Utc};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

// Why: Cache TTL for negative results (jti not revoked). Revoked results are
// cached indefinitely because revocation is monotonic — a revoked jti cannot
// become un-revoked. 60s caps the window during which a freshly-revoked
// token continues to pass.
const NEGATIVE_TTL_SECONDS: u64 = 60;

const DEFAULT_CACHE_CAPACITY: usize = 5_000;

impl OAuthRepository {
    pub async fn revoke_jti(
        &self,
        jti: &str,
        user_id: Uuid,
        exp: DateTime<Utc>,
    ) -> OauthResult<()> {
        sqlx::query!(
            "INSERT INTO oauth_jti_revocations (jti, user_id, exp)
             VALUES ($1, $2, $3)
             ON CONFLICT (jti) DO NOTHING",
            jti,
            user_id,
            exp,
        )
        .execute(self.write_pool_ref())
        .await?;
        Ok(())
    }

    pub async fn is_jti_revoked(&self, jti: &str) -> OauthResult<bool> {
        let revoked = sqlx::query_scalar!(
            r#"SELECT EXISTS(
                 SELECT 1 FROM oauth_jti_revocations
                  WHERE jti = $1 AND exp > now()
               ) AS "exists!""#,
            jti,
        )
        .fetch_one(self.pool_ref())
        .await?;
        Ok(revoked)
    }

    /// Admin "kick user" — caller passes the max expiry across the user's
    /// outstanding access tokens. The middleware sees a revoked jti per row
    /// inserted. For revoking a *fleet* of jtis (rotation across many active
    /// sessions) the caller is expected to assemble the list of jtis from
    /// session history first; this method is a thin transactional batch.
    pub async fn revoke_jtis_for_user(
        &self,
        user_id: Uuid,
        jtis: &[String],
        exp_floor: DateTime<Utc>,
    ) -> OauthResult<u64> {
        let mut inserted: u64 = 0;
        for jti in jtis {
            let result = sqlx::query!(
                "INSERT INTO oauth_jti_revocations (jti, user_id, exp)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (jti) DO NOTHING",
                jti,
                user_id,
                exp_floor,
            )
            .execute(self.write_pool_ref())
            .await?;
            inserted += result.rows_affected();
        }
        Ok(inserted)
    }

    pub async fn cleanup_expired_jti_revocations(&self) -> OauthResult<u64> {
        let result = sqlx::query!("DELETE FROM oauth_jti_revocations WHERE exp < now()")
            .execute(self.write_pool_ref())
            .await?;
        Ok(result.rows_affected())
    }
}

#[derive(Debug, Clone, Copy)]
enum CacheEntry {
    /// Negative result with insertion instant — expires after
    /// [`NEGATIVE_TTL_SECONDS`].
    NotRevoked { inserted_at: Instant },
    /// Revocation is monotonic, so this entry is held until the LRU evicts it.
    Revoked,
}

/// In-memory LRU layered in front of `is_jti_revoked`. Negative results
/// expire after 60s; positive results are sticky (a revoked jti cannot
/// become un-revoked).
pub struct JtiRevocationCache {
    cache: Mutex<LruCache<String, CacheEntry>>,
}

impl std::fmt::Debug for JtiRevocationCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JtiRevocationCache").finish_non_exhaustive()
    }
}

impl JtiRevocationCache {
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CACHE_CAPACITY)
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity.max(1)).unwrap_or(NonZeroUsize::MIN);
        Self {
            cache: Mutex::new(LruCache::new(cap)),
        }
    }

    /// `None` → not in cache (caller must hit the DB).
    /// `Some(true)` → revoked. `Some(false)` → fresh negative.
    pub fn peek(&self, jti: &str) -> Option<bool> {
        let mut guard = self.cache.lock().ok()?;
        match guard.get(jti).copied()? {
            CacheEntry::Revoked => Some(true),
            CacheEntry::NotRevoked { inserted_at } => {
                if inserted_at.elapsed() < Duration::from_secs(NEGATIVE_TTL_SECONDS) {
                    Some(false)
                } else {
                    guard.pop(jti);
                    None
                }
            },
        }
    }

    pub fn record(&self, jti: &str, revoked: bool) {
        if let Ok(mut guard) = self.cache.lock() {
            let entry = if revoked {
                CacheEntry::Revoked
            } else {
                CacheEntry::NotRevoked {
                    inserted_at: Instant::now(),
                }
            };
            guard.put(jti.to_owned(), entry);
        }
    }
}

impl Default for JtiRevocationCache {
    fn default() -> Self {
        Self::new()
    }
}
