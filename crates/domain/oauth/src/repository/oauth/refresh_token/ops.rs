//! Refresh-token rotation, family revocation, and replay handling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use systemprompt_identifiers::{ApiKeyId, ClientId, RefreshTokenFamilyId, RefreshTokenId, UserId};

use super::{ConsumedRefreshToken, RefreshTokenHolder, RefreshTokenLineage, RefreshTokenParams};
use crate::error::{OauthError, OauthResult, ValidationError};
use crate::repository::oauth::OAuthRepository;

impl OAuthRepository {
    pub async fn store_refresh_token(&self, params: RefreshTokenParams<'_>) -> OauthResult<()> {
        let expires_at_dt = chrono::DateTime::<Utc>::from_timestamp(params.expires_at, 0)
            .ok_or_else(|| {
                OauthError::Validation(ValidationError::Rejected {
                    reason: "Invalid timestamp for expires_at",
                })
            })?;
        let now = Utc::now();
        let token_id_hash = self.hash_at_rest(params.token_id.as_str())?;
        let client_id = params.client_id.as_str();
        let user_id = params.user_id.as_str();

        match params.lineage {
            RefreshTokenLineage::Family(family_id) => {
                sqlx::query!(
                    "INSERT INTO oauth_refresh_tokens (token_id, client_id, user_id, scope, \
                     expires_at, created_at, family_id, resource, authenticated_at, \
                     session_started_at, device_key_id)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                             (SELECT device_key_id FROM oauth_refresh_tokens
                              WHERE family_id = $7 AND device_key_id IS NOT NULL LIMIT 1))",
                    token_id_hash,
                    client_id,
                    user_id,
                    params.scope,
                    expires_at_dt,
                    now,
                    family_id.as_str(),
                    params.resource,
                    params.authenticated_at,
                    params.session_started_at
                )
                .execute(self.write_pool_ref())
                .await?;
            },
            RefreshTokenLineage::AuthorizationCode(code) => {
                let code_hash = self.hash_at_rest(code.as_str())?;
                let inserted = sqlx::query!(
                    "INSERT INTO oauth_refresh_tokens (token_id, client_id, user_id, scope, \
                     expires_at, created_at, family_id, resource, authenticated_at, \
                     session_started_at, device_key_id)
                     SELECT $1, $2, $3, $4, $5, $6, $1, $7, $9, $10, device_key_id
                     FROM oauth_auth_codes
                     WHERE code = $8 AND refresh_token_id = $1
                     FOR UPDATE",
                    token_id_hash,
                    client_id,
                    user_id,
                    params.scope,
                    expires_at_dt,
                    now,
                    params.resource,
                    code_hash,
                    params.authenticated_at,
                    params.session_started_at
                )
                .execute(self.write_pool_ref())
                .await?;
                if inserted.rows_affected() == 0 {
                    return Err(OauthError::Validation(ValidationError::Rejected {
                        reason: "Invalid authorization code",
                    }));
                }
            },
        }
        Ok(())
    }

    pub async fn refresh_token_resource(
        &self,
        token_id: &RefreshTokenId,
        client_id: &ClientId,
    ) -> OauthResult<Option<String>> {
        let token_id_hash = self.hash_at_rest(token_id.as_str())?;
        let resource = sqlx::query_scalar!(
            "SELECT resource FROM oauth_refresh_tokens WHERE token_id = $1 AND client_id = $2",
            token_id_hash,
            client_id.as_str()
        )
        .fetch_optional(self.write_pool_ref())
        .await?;
        Ok(resource.flatten())
    }

    pub async fn refresh_token_device_key(
        &self,
        token_id: &RefreshTokenId,
        client_id: &ClientId,
    ) -> OauthResult<Option<ApiKeyId>> {
        let token_id_hash = self.hash_at_rest(token_id.as_str())?;
        let device_key = sqlx::query_scalar!(
            "SELECT device_key_id FROM oauth_refresh_tokens WHERE token_id = $1 AND client_id = $2",
            token_id_hash,
            client_id.as_str()
        )
        .fetch_optional(self.write_pool_ref())
        .await?;
        Ok(device_key.flatten().map(ApiKeyId::new))
    }

    pub async fn revoke_refresh_token_family_of(
        &self,
        token_id: &RefreshTokenId,
    ) -> OauthResult<u64> {
        let token_id_hash = self.hash_at_rest(token_id.as_str())?;
        let result = sqlx::query!(
            "DELETE FROM oauth_refresh_tokens
             WHERE family_id = (SELECT family_id FROM oauth_refresh_tokens WHERE token_id = $1)",
            token_id_hash
        )
        .execute(self.write_pool_ref())
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn consume_refresh_token(
        &self,
        token_id: &RefreshTokenId,
        client_id: &ClientId,
    ) -> OauthResult<ConsumedRefreshToken> {
        let now = Utc::now();
        let token_id_hash = self.hash_at_rest(token_id.as_str())?;
        let client_id_str = client_id.as_str();

        let claimed = sqlx::query!(
            "UPDATE oauth_refresh_tokens
             SET consumed_at = $1
             WHERE token_id = $2 AND client_id = $3
               AND consumed_at IS NULL
               AND expires_at >= $1
             RETURNING user_id, scope, family_id, authenticated_at, session_started_at",
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
                family_id: RefreshTokenFamilyId::new(row.family_id),
                authenticated_at: row.authenticated_at,
                session_started_at: row.session_started_at,
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
        .fetch_optional(self.write_pool_ref())
        .await?;

        let Some(row) = row else {
            return Err(OauthError::TokenInvalid {
                reason: "Invalid refresh token",
            });
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
            return Err(OauthError::TokenInvalid {
                reason: "Invalid refresh token",
            });
        }

        if row.expires_at < Utc::now() {
            return Err(OauthError::Expired {
                reason: "Refresh token expired",
            });
        }

        Err(OauthError::TokenInvalid {
            reason: "Invalid refresh token",
        })
    }

    pub async fn revoke_refresh_token(&self, token_id: &RefreshTokenId) -> OauthResult<bool> {
        let token_id_hash = self.hash_at_rest(token_id.as_str())?;
        let result = sqlx::query!(
            "DELETE FROM oauth_refresh_tokens WHERE token_id = $1",
            token_id_hash
        )
        .execute(self.write_pool_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn find_client_id_from_refresh_token(
        &self,
        token_id: &RefreshTokenId,
    ) -> OauthResult<Option<ClientId>> {
        let token_id_hash = self.hash_at_rest(token_id.as_str())?;
        let result = sqlx::query_scalar!(
            "SELECT client_id FROM oauth_refresh_tokens WHERE token_id = $1",
            token_id_hash
        )
        .fetch_optional(self.write_pool_ref())
        .await?;

        Ok(result.map(ClientId::new))
    }

    pub async fn find_refresh_token_holder(
        &self,
        token_id: &RefreshTokenId,
    ) -> OauthResult<Option<RefreshTokenHolder>> {
        let token_id_hash = self.hash_at_rest(token_id.as_str())?;
        let row = sqlx::query!(
            "SELECT client_id, user_id FROM oauth_refresh_tokens WHERE token_id = $1",
            token_id_hash
        )
        .fetch_optional(self.write_pool_ref())
        .await?;

        Ok(row.map(|row| RefreshTokenHolder {
            client_id: ClientId::new(row.client_id),
            user_id: UserId::new(row.user_id),
        }))
    }
}
