//! Refresh-token rotation, family revocation, and replay handling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::{ClientId, RefreshTokenId, UserId};

use super::{ConsumedRefreshToken, RefreshTokenParams};
use crate::error::{OauthError, OauthResult};
use crate::repository::oauth::OAuthRepository;
use crate::repository::oauth::at_rest::hash_at_rest;

impl OAuthRepository {
    pub async fn store_refresh_token(&self, params: RefreshTokenParams<'_>) -> OauthResult<()> {
        let expires_at_dt = chrono::DateTime::<Utc>::from_timestamp(params.expires_at, 0)
            .ok_or_else(|| OauthError::Validation("Invalid timestamp for expires_at".to_owned()))?;
        let now = Utc::now();
        let token_id_hash = hash_at_rest(params.token_id.as_str())?;
        let client_id = params.client_id.as_str();
        let user_id = params.user_id.as_str();
        let family_id_owned = token_id_hash.clone();
        let family_id: &str = params.family_id.unwrap_or(&family_id_owned);

        sqlx::query!(
            "INSERT INTO oauth_refresh_tokens (token_id, client_id, user_id, scope, expires_at, \
             created_at, family_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            token_id_hash,
            client_id,
            user_id,
            params.scope,
            expires_at_dt,
            now,
            family_id
        )
        .execute(self.write_pool_ref())
        .await?;

        Ok(())
    }

    /// Returns the family regardless of whether the token has been consumed —
    /// callers use this to revoke the family on replay. `None` means the
    /// token does not exist.
    pub async fn find_refresh_token_family(
        &self,
        token_id: &RefreshTokenId,
    ) -> OauthResult<Option<String>> {
        let token_id_hash = hash_at_rest(token_id.as_str())?;
        let result = sqlx::query_scalar!(
            "SELECT family_id FROM oauth_refresh_tokens WHERE token_id = $1",
            token_id_hash
        )
        .fetch_optional(self.pool_ref())
        .await?;
        Ok(result)
    }

    /// Deletes both active and consumed-tombstone rows for the family.
    pub async fn revoke_refresh_token_family(&self, family_id: &str) -> OauthResult<u64> {
        let result = sqlx::query!(
            "DELETE FROM oauth_refresh_tokens WHERE family_id = $1",
            family_id
        )
        .execute(self.write_pool_ref())
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn validate_refresh_token(
        &self,
        token_id: &RefreshTokenId,
        client_id: &ClientId,
    ) -> OauthResult<(UserId, String)> {
        let now = Utc::now();
        let token_id_hash = hash_at_rest(token_id.as_str())?;
        let client_id_str = client_id.as_str();

        let row = sqlx::query!(
            "SELECT user_id, scope, expires_at, consumed_at FROM oauth_refresh_tokens
             WHERE token_id = $1 AND client_id = $2",
            token_id_hash,
            client_id_str
        )
        .fetch_optional(self.pool_ref())
        .await?
        .ok_or_else(|| OauthError::TokenInvalid("Invalid refresh token".to_owned()))?;

        if row.consumed_at.is_some() {
            return Err(OauthError::TokenInvalid("Invalid refresh token".to_owned()));
        }

        if row.expires_at < now {
            return Err(OauthError::Expired("Refresh token expired".to_owned()));
        }

        Ok((UserId::new(row.user_id), row.scope))
    }

    /// Atomically claim a refresh token for rotation. Race-loser receives
    /// [`OauthError::TokenInvalid`]. If the unclaimable token is a replay
    /// (already consumed), its family is revoked transparently before the
    /// error is returned (RFC 6819 §5.2.2.3 refresh-token-rotation).
    pub async fn consume_refresh_token(
        &self,
        token_id: &RefreshTokenId,
        client_id: &ClientId,
    ) -> OauthResult<ConsumedRefreshToken> {
        let now = Utc::now();
        let token_id_hash = hash_at_rest(token_id.as_str())?;
        let client_id_str = client_id.as_str();

        let claimed = sqlx::query!(
            "UPDATE oauth_refresh_tokens
             SET consumed_at = $1
             WHERE token_id = $2 AND client_id = $3
               AND consumed_at IS NULL
               AND expires_at >= $1
             RETURNING user_id, scope, family_id",
            now,
            token_id_hash,
            client_id_str
        )
        .fetch_optional(self.write_pool_ref())
        .await?;

        if let Some(row) = claimed {
            return Ok(ConsumedRefreshToken {
                user_id: UserId::new(row.user_id),
                scope: row.scope,
                family_id: row.family_id,
            });
        }

        self.handle_unclaimable_refresh_token(&token_id_hash).await
    }

    async fn handle_unclaimable_refresh_token(
        &self,
        token_id_hash: &str,
    ) -> OauthResult<ConsumedRefreshToken> {
        let row = sqlx::query!(
            "SELECT family_id, consumed_at, expires_at
             FROM oauth_refresh_tokens WHERE token_id = $1",
            token_id_hash
        )
        .fetch_optional(self.pool_ref())
        .await?;

        let Some(row) = row else {
            return Err(OauthError::TokenInvalid("Invalid refresh token".to_owned()));
        };

        if row.consumed_at.is_some() {
            let family_id = row.family_id;
            let result = sqlx::query!(
                "DELETE FROM oauth_refresh_tokens WHERE family_id = $1",
                family_id
            )
            .execute(self.write_pool_ref())
            .await?;
            tracing::warn!(
                event = "refresh_token_reuse_detected",
                family_id = %family_id,
                revoked_count = result.rows_affected(),
                "Revoked refresh-token family after refresh-token replay"
            );
            return Err(OauthError::TokenInvalid("Invalid refresh token".to_owned()));
        }

        if row.expires_at < Utc::now() {
            return Err(OauthError::Expired("Refresh token expired".to_owned()));
        }

        Err(OauthError::TokenInvalid("Invalid refresh token".to_owned()))
    }

    pub async fn revoke_refresh_token(&self, token_id: &RefreshTokenId) -> OauthResult<bool> {
        let token_id_hash = hash_at_rest(token_id.as_str())?;
        let result = sqlx::query!(
            "DELETE FROM oauth_refresh_tokens WHERE token_id = $1",
            token_id_hash
        )
        .execute(self.write_pool_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Live tokens (`consumed_at` IS NULL, `expires_at` >= now) and
    /// recently-consumed tombstones (still within their original expiry
    /// window) are preserved so replay detection retains evidence.
    pub async fn cleanup_expired_refresh_tokens(&self) -> OauthResult<u64> {
        let now = Utc::now();

        let result = sqlx::query!(
            "DELETE FROM oauth_refresh_tokens WHERE expires_at < $1",
            now
        )
        .execute(self.write_pool_ref())
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn find_client_id_from_refresh_token(
        &self,
        token_id: &RefreshTokenId,
    ) -> OauthResult<Option<ClientId>> {
        let token_id_hash = hash_at_rest(token_id.as_str())?;
        let result = sqlx::query_scalar!(
            "SELECT client_id FROM oauth_refresh_tokens WHERE token_id = $1",
            token_id_hash
        )
        .fetch_optional(self.pool_ref())
        .await?;

        Ok(result.map(ClientId::new))
    }
}
