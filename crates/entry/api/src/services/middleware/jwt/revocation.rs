//! JTI revocation gate for the JWT context extractor.
//!
//! Runs as the final stateful check after a token's claims, its backing user,
//! and the session row have all validated. It answers the one question
//! signature validation cannot: has this specific token been explicitly
//! revoked (logout, admin revoke, refresh rotation)? A negative result is
//! cached so the hot path costs one map lookup. Fails closed — a revocation
//! store error rejects the request rather than admitting an unverifiable token.

use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::execution::context::ContextExtractionError;
use systemprompt_oauth::OauthResult;
use systemprompt_oauth::repository::{JtiRevocationCache, OAuthRepository};

#[derive(Clone)]
pub struct JtiRevocationChecker {
    repo: Arc<OAuthRepository>,
    cache: Arc<JtiRevocationCache>,
}

impl std::fmt::Debug for JtiRevocationChecker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JtiRevocationChecker")
            .finish_non_exhaustive()
    }
}

impl JtiRevocationChecker {
    pub fn from_pool(db: &DbPool) -> OauthResult<Self> {
        Ok(Self {
            repo: Arc::new(OAuthRepository::new(db)?),
            cache: Arc::new(JtiRevocationCache::new()),
        })
    }

    pub async fn ensure_not_revoked(&self, jti: &str) -> Result<(), ContextExtractionError> {
        if jti.is_empty() {
            return Ok(());
        }
        match self.cache.peek(jti) {
            Some(true) => return Err(ContextExtractionError::Revoked),
            Some(false) => return Ok(()),
            None => {},
        }

        let revoked = self.repo.is_jti_revoked(jti).await.map_err(|e| {
            ContextExtractionError::DatabaseError(format!("JTI revocation lookup failed: {e}"))
        })?;
        self.cache.record(jti, revoked);
        if revoked {
            Err(ContextExtractionError::Revoked)
        } else {
            Ok(())
        }
    }
}
