//! Refresh token persistence and rotation.

use super::OAuthRepository;
use crate::error::{OauthError, OauthResult};
use chrono::Utc;
use systemprompt_identifiers::{ClientId, RefreshTokenId, UserId};

#[derive(Debug)]
pub struct RefreshTokenParams<'a> {
    pub token_id: &'a RefreshTokenId,
    pub client_id: &'a ClientId,
    pub user_id: &'a UserId,
    pub scope: &'a str,
    pub expires_at: i64,
    /// Family-identifier shared by every refresh token derived from the same
    /// initial authorization-code exchange. When `None`, the family is seeded
    /// from `token_id` (first issuance). Subsequent rotations carry the parent
    /// token's family forward so a single auth-code-replay detection can
    /// invalidate every descendant.
    pub family_id: Option<&'a str>,
}

#[derive(Debug)]
pub struct RefreshTokenParamsBuilder<'a> {
    token_id: &'a RefreshTokenId,
    client_id: &'a ClientId,
    user_id: &'a UserId,
    scope: &'a str,
    expires_at: i64,
    family_id: Option<&'a str>,
}

impl<'a> RefreshTokenParamsBuilder<'a> {
    pub const fn new(
        token_id: &'a RefreshTokenId,
        client_id: &'a ClientId,
        user_id: &'a UserId,
        scope: &'a str,
        expires_at: i64,
    ) -> Self {
        Self {
            token_id,
            client_id,
            user_id,
            scope,
            expires_at,
            family_id: None,
        }
    }

    pub const fn with_family(mut self, family_id: &'a str) -> Self {
        self.family_id = Some(family_id);
        self
    }

    pub const fn build(self) -> RefreshTokenParams<'a> {
        RefreshTokenParams {
            token_id: self.token_id,
            client_id: self.client_id,
            user_id: self.user_id,
            scope: self.scope,
            expires_at: self.expires_at,
            family_id: self.family_id,
        }
    }
}

impl<'a> RefreshTokenParams<'a> {
    pub const fn builder(
        token_id: &'a RefreshTokenId,
        client_id: &'a ClientId,
        user_id: &'a UserId,
        scope: &'a str,
        expires_at: i64,
    ) -> RefreshTokenParamsBuilder<'a> {
        RefreshTokenParamsBuilder::new(token_id, client_id, user_id, scope, expires_at)
    }
}

impl OAuthRepository {
    pub async fn store_refresh_token(&self, params: RefreshTokenParams<'_>) -> OauthResult<()> {
        let expires_at_dt = chrono::DateTime::<Utc>::from_timestamp(params.expires_at, 0)
            .ok_or_else(|| {
                OauthError::Validation("Invalid timestamp for expires_at".to_string())
            })?;
        let now = Utc::now();
        let token_id = params.token_id.as_str();
        let client_id = params.client_id.as_str();
        let user_id = params.user_id.as_str();
        let family_id = params.family_id.unwrap_or(token_id);

        sqlx::query!(
            "INSERT INTO oauth_refresh_tokens (token_id, client_id, user_id, scope, expires_at, \
             created_at, family_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            token_id,
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

    /// Fetch a refresh-token's family id (returns `None` if the token does not
    /// exist).
    pub async fn get_refresh_token_family(
        &self,
        token_id: &RefreshTokenId,
    ) -> OauthResult<Option<String>> {
        let token_id_str = token_id.as_str();
        let result = sqlx::query_scalar!(
            "SELECT family_id FROM oauth_refresh_tokens WHERE token_id = $1",
            token_id_str
        )
        .fetch_optional(self.pool_ref())
        .await?;
        Ok(result)
    }

    /// Revoke every refresh token in a family. Returns the number deleted.
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
        let token_id_str = token_id.as_str();
        let client_id_str = client_id.as_str();

        let row = sqlx::query!(
            "SELECT user_id, scope, expires_at FROM oauth_refresh_tokens
             WHERE token_id = $1 AND client_id = $2",
            token_id_str,
            client_id_str
        )
        .fetch_optional(self.pool_ref())
        .await?
        .ok_or_else(|| OauthError::Token("Invalid refresh token".to_string()))?;

        if row.expires_at < now {
            return Err(OauthError::Token("Refresh token expired".to_string()));
        }

        Ok((UserId::new(row.user_id), row.scope))
    }

    pub async fn consume_refresh_token(
        &self,
        token_id: &RefreshTokenId,
        client_id: &ClientId,
    ) -> OauthResult<ConsumedRefreshToken> {
        let (user_id, scope) = self.validate_refresh_token(token_id, client_id).await?;
        let token_id_str = token_id.as_str();

        let row = sqlx::query!(
            "DELETE FROM oauth_refresh_tokens WHERE token_id = $1 RETURNING family_id",
            token_id_str
        )
        .fetch_one(self.write_pool_ref())
        .await?;

        Ok(ConsumedRefreshToken {
            user_id,
            scope,
            family_id: row.family_id,
        })
    }

    pub async fn revoke_refresh_token(&self, token_id: &RefreshTokenId) -> OauthResult<bool> {
        let token_id_str = token_id.as_str();
        let result = sqlx::query!(
            "DELETE FROM oauth_refresh_tokens WHERE token_id = $1",
            token_id_str
        )
        .execute(self.write_pool_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

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

    pub async fn get_client_id_from_refresh_token(
        &self,
        token_id: &RefreshTokenId,
    ) -> OauthResult<Option<ClientId>> {
        let token_id_str = token_id.as_str();
        let result = sqlx::query_scalar!(
            "SELECT client_id FROM oauth_refresh_tokens WHERE token_id = $1",
            token_id_str
        )
        .fetch_optional(self.pool_ref())
        .await?;

        Ok(result.map(ClientId::new))
    }
}

#[derive(Debug)]
pub struct ConsumedRefreshToken {
    pub user_id: UserId,
    pub scope: String,
    pub family_id: String,
}
