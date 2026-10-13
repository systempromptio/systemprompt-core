//! Single-use store for SSO assertion ids. Cross-instance authoritative, so a
//! claim is an atomic `INSERT ... ON CONFLICT DO NOTHING` keyed on
//! `(scope, id)`; [`PostgresReplayGuard`] exposes it as the
//! [`ReplayGuard`] the SSO driver calls.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_traits::sso::{ReplayGuard, SsoError};

use super::OAuthRepository;
use crate::error::OauthResult;

impl OAuthRepository {
    /// `true` the first time `(scope, id)` is claimed.
    pub async fn claim_sso_replay(
        &self,
        scope: &str,
        id: &str,
        expires_at: DateTime<Utc>,
    ) -> OauthResult<bool> {
        let result = sqlx::query(
            "INSERT INTO sso_replay_guard (scope, id, expires_at)
             VALUES ($1, $2, $3)
             ON CONFLICT (scope, id) DO NOTHING",
        )
        .bind(scope)
        .bind(id)
        .bind(expires_at)
        .execute(self.write_pool_ref())
        .await?;
        Ok(result.rows_affected() == 1)
    }
}

#[derive(Debug, Clone)]
pub struct PostgresReplayGuard {
    repo: OAuthRepository,
}

impl PostgresReplayGuard {
    #[must_use]
    pub const fn new(repo: OAuthRepository) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl ReplayGuard for PostgresReplayGuard {
    async fn claim(
        &self,
        scope: &str,
        id: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<bool, SsoError> {
        self.repo
            .claim_sso_replay(scope, id, expires_at)
            .await
            .map_err(SsoError::internal)
    }
}
