use super::OAuthRepository;
use anyhow::Result;
use chrono::Utc;
use systemprompt_identifiers::{ClientId, RefreshTokenId, UserId};

#[derive(Debug)]
pub struct RefreshTokenParams<'a> {
    pub token_id: &'a RefreshTokenId,
    pub client_id: &'a ClientId,
    pub user_id: &'a UserId,
    pub scope: &'a str,
    pub expires_at: i64,
}

#[derive(Debug)]
pub struct RefreshTokenParamsBuilder<'a> {
    token_id: &'a RefreshTokenId,
    client_id: &'a ClientId,
    user_id: &'a UserId,
    scope: &'a str,
    expires_at: i64,
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
        }
    }

    pub const fn build(self) -> RefreshTokenParams<'a> {
        RefreshTokenParams {
            token_id: self.token_id,
            client_id: self.client_id,
            user_id: self.user_id,
            scope: self.scope,
            expires_at: self.expires_at,
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
    pub async fn store_refresh_token(&self, params: RefreshTokenParams<'_>) -> Result<()> {
        let expires_at_dt = chrono::DateTime::<Utc>::from_timestamp(params.expires_at, 0)
            .ok_or_else(|| anyhow::anyhow!("Invalid timestamp for expires_at"))?;
        let now = Utc::now();
        let token_id = params.token_id.as_str();
        let client_id = params.client_id.as_str();
        let user_id = params.user_id.as_str();

        sqlx::query!(
            "INSERT INTO oauth_refresh_tokens (token_id, client_id, user_id, scope, expires_at, \
             created_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
            token_id,
            client_id,
            user_id,
            params.scope,
            expires_at_dt,
            now
        )
        .execute(self.write_pool_ref())
        .await?;

        Ok(())
    }

    pub async fn validate_refresh_token(
        &self,
        token_id: &RefreshTokenId,
        client_id: &ClientId,
    ) -> Result<(UserId, String)> {
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
        .ok_or_else(|| anyhow::anyhow!("Invalid refresh token"))?;

        if row.expires_at < now {
            return Err(anyhow::anyhow!("Refresh token expired"));
        }

        Ok((UserId::new(row.user_id), row.scope))
    }

    pub async fn consume_refresh_token(
        &self,
        token_id: &RefreshTokenId,
        client_id: &ClientId,
    ) -> Result<(UserId, String)> {
        let (user_id, scope) = self.validate_refresh_token(token_id, client_id).await?;
        let token_id_str = token_id.as_str();

        sqlx::query!(
            "DELETE FROM oauth_refresh_tokens WHERE token_id = $1",
            token_id_str
        )
        .execute(self.write_pool_ref())
        .await?;

        Ok((user_id, scope))
    }

    pub async fn revoke_refresh_token(&self, token_id: &RefreshTokenId) -> Result<bool> {
        let token_id_str = token_id.as_str();
        let result = sqlx::query!(
            "DELETE FROM oauth_refresh_tokens WHERE token_id = $1",
            token_id_str
        )
        .execute(self.write_pool_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn cleanup_expired_refresh_tokens(&self) -> Result<u64> {
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
    ) -> Result<Option<ClientId>> {
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
