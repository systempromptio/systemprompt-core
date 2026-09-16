//! Expiry sweeps over the OAuth-owned tables: refresh tokens, authorization
//! codes, state bindings, JTI revocations and ID-JAG replay markers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;

use crate::error::OauthResult;

#[derive(Debug, Clone)]
pub struct OauthCleanupRepository {
    write_pool: Arc<PgPool>,
}

/// Rows removed by one [`OauthCleanupRepository::delete_expired`] sweep.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OauthCleanupCounts {
    pub codes: u64,
    pub tokens: u64,
    pub state_bindings: u64,
    pub jti_revocations: u64,
    pub id_jag_replays: u64,
}

impl OauthCleanupCounts {
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.codes + self.tokens + self.state_bindings + self.jti_revocations + self.id_jag_replays
    }
}

impl OauthCleanupRepository {
    pub fn new(db: &DbPool) -> OauthResult<Self> {
        Ok(Self {
            write_pool: db.write_pool_arc()?,
        })
    }

    pub async fn delete_expired(&self) -> OauthResult<OauthCleanupCounts> {
        Ok(OauthCleanupCounts {
            codes: self.delete_expired_auth_codes().await?,
            tokens: self.delete_expired_refresh_tokens().await?,
            state_bindings: self.delete_expired_state_bindings().await?,
            jti_revocations: self.delete_expired_jti_revocations().await?,
            id_jag_replays: self.delete_expired_id_jag_replays().await?,
        })
    }

    pub async fn delete_expired_refresh_tokens(&self) -> OauthResult<u64> {
        let result = sqlx::query!("DELETE FROM oauth_refresh_tokens WHERE expires_at < NOW()")
            .execute(&*self.write_pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_expired_auth_codes(&self) -> OauthResult<u64> {
        let result = sqlx::query!(
            "DELETE FROM oauth_auth_codes WHERE expires_at < NOW() OR used_at IS NOT NULL"
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_expired_state_bindings(&self) -> OauthResult<u64> {
        let result = sqlx::query!("DELETE FROM oauth_state_bindings WHERE expires_at < NOW()")
            .execute(&*self.write_pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_expired_jti_revocations(&self) -> OauthResult<u64> {
        let result = sqlx::query!("DELETE FROM oauth_jti_revocations WHERE exp < NOW()")
            .execute(&*self.write_pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_expired_id_jag_replays(&self) -> OauthResult<u64> {
        let result = sqlx::query!("DELETE FROM id_jag_replay WHERE expires_at < NOW()")
            .execute(&*self.write_pool)
            .await?;
        Ok(result.rows_affected())
    }
}
