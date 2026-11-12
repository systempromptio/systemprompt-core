//! Authorisation code persistence with mandatory PKCE. Every code is bound to
//! an S256 challenge and its redirect URI, and redemption always checks both.
//! The `code` and the linked `refresh_token_id` are stored as HMAC-SHA-256
//! digests under the deployment pepper; raw values never touch the database.
//! Redemption records the refresh token it mints in the same statement that
//! claims the code, so a replay always finds the family to revoke.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OAuthRepository;
use crate::error::{OauthError, OauthResult};
use crate::models::PkceMethod;
use chrono::Utc;
use pkce::verify_pkce;
use systemprompt_identifiers::{AuthorizationCode, ClientId, UserId};

mod params;
mod pkce;

pub use params::{
    AuthCodeParams, AuthCodeRedemption, AuthCodeValidationResult, MintAuthCodeParams,
};

impl OAuthRepository {
    pub async fn mint_authorization_code(
        &self,
        params: MintAuthCodeParams<'_>,
    ) -> OauthResult<AuthorizationCode> {
        let scope = params.scope.map_or_else(
            || {
                let default_roles = Self::get_default_roles();
                if default_roles.is_empty() {
                    "user".to_owned()
                } else {
                    default_roles.join(" ")
                }
            },
            str::to_owned,
        );
        if params.code_challenge.is_empty() {
            return Err(OauthError::Validation(
                "PKCE code_challenge is required".to_owned(),
            ));
        }
        if params.code_challenge_method.parse::<PkceMethod>().is_err() {
            return Err(OauthError::Validation(
                "PKCE code_challenge_method must be S256".to_owned(),
            ));
        }
        let code = AuthorizationCode::new(crate::services::generate_secure_token("auth_code"));

        self.store_authorization_code(AuthCodeParams {
            code: &code,
            client_id: params.client_id,
            user_id: params.user_id,
            redirect_uri: params.redirect_uri,
            scope: &scope,
            code_challenge: params.code_challenge,
            resource: params.resource,
        })
        .await?;
        Ok(code)
    }
    pub async fn store_authorization_code(&self, params: AuthCodeParams<'_>) -> OauthResult<()> {
        if params.code_challenge.is_empty() {
            return Err(OauthError::Validation(
                "PKCE code_challenge is required".to_owned(),
            ));
        }
        let code_challenge_method = PkceMethod::S256.as_str();
        let expires_at = Utc::now() + chrono::Duration::seconds(600);
        let now = Utc::now();
        let code_hash = self.hash_at_rest(params.code.as_str())?;
        let client_id = params.client_id.as_str();
        let user_id = params.user_id.as_str();

        sqlx::query!(
            "INSERT INTO oauth_auth_codes
             (code, client_id, user_id, redirect_uri, scope, expires_at, code_challenge,
             code_challenge_method, resource, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            code_hash,
            client_id,
            user_id,
            params.redirect_uri,
            params.scope,
            expires_at,
            params.code_challenge,
            code_challenge_method,
            params.resource,
            now
        )
        .execute(self.write_pool_ref())
        .await?;

        Ok(())
    }

    pub async fn find_client_id_from_auth_code(
        &self,
        code: &AuthorizationCode,
    ) -> OauthResult<Option<ClientId>> {
        let code_hash = self.hash_at_rest(code.as_str())?;
        let result = sqlx::query_scalar!(
            "SELECT client_id FROM oauth_auth_codes WHERE code = $1",
            code_hash
        )
        .fetch_optional(self.write_pool_ref())
        .await?;

        Ok(result.map(ClientId::new))
    }

    pub async fn validate_authorization_code(
        &self,
        redemption: AuthCodeRedemption<'_>,
    ) -> OauthResult<AuthCodeValidationResult> {
        let AuthCodeRedemption {
            code,
            client_id,
            redirect_uri,
            code_verifier,
            refresh_token_id,
        } = redemption;
        let now = Utc::now();
        let code_hash = self.hash_at_rest(code.as_str())?;
        let refresh_token_id_hash = self.hash_at_rest(refresh_token_id.as_str())?;

        let claimed = sqlx::query!(
            "UPDATE oauth_auth_codes
             SET used_at = $1, refresh_token_id = $3
             WHERE code = $2 AND used_at IS NULL
             RETURNING client_id, user_id, scope, expires_at, redirect_uri, code_challenge,
                       code_challenge_method, resource",
            now,
            code_hash,
            refresh_token_id_hash
        )
        .fetch_optional(self.write_pool_ref())
        .await?;

        let Some(row) = claimed else {
            return self.handle_unclaimable_auth_code(&code_hash).await;
        };

        if row.client_id != client_id.as_str() {
            tracing::warn!(
                expected = %row.client_id,
                actual = %client_id,
                "Authorization code redeemed by a different client"
            );
            return Err(OauthError::Validation(
                "Invalid authorization code".to_owned(),
            ));
        }

        if row.expires_at < now {
            tracing::warn!("Authorization code expired");
            return Err(OauthError::Validation(
                "Invalid authorization code".to_owned(),
            ));
        }

        if row.redirect_uri != redirect_uri {
            tracing::warn!(
                expected = %row.redirect_uri,
                actual = %redirect_uri,
                "Redirect URI mismatch"
            );
            return Err(OauthError::Validation(
                "Invalid authorization code".to_owned(),
            ));
        }

        let Some(challenge) = row.code_challenge.as_deref() else {
            tracing::warn!("Authorization code carries no PKCE challenge");
            return Err(OauthError::Validation(
                "Invalid authorization code".to_owned(),
            ));
        };
        verify_pkce(
            challenge,
            row.code_challenge_method.as_deref(),
            code_verifier,
        )?;

        Ok(AuthCodeValidationResult {
            user_id: UserId::new(row.user_id),
            scope: row.scope,
            resource: row.resource,
        })
    }

    async fn handle_unclaimable_auth_code(
        &self,
        code_hash: &str,
    ) -> OauthResult<AuthCodeValidationResult> {
        let used_at = sqlx::query_scalar!(
            "SELECT used_at FROM oauth_auth_codes WHERE code = $1",
            code_hash
        )
        .fetch_optional(self.write_pool_ref())
        .await?;

        match used_at {
            None => tracing::warn!("Authorization code not found"),
            Some(None) => tracing::warn!("Authorization code could not be claimed"),
            Some(Some(_)) => self.revoke_auth_code_family(code_hash).await?,
        }

        Err(OauthError::Validation(
            "Invalid authorization code".to_owned(),
        ))
    }

    async fn revoke_auth_code_family(&self, code_hash: &str) -> OauthResult<()> {
        let family = sqlx::query_scalar!(
            r#"WITH prior AS (
                   SELECT code, refresh_token_id FROM oauth_auth_codes
                   WHERE code = $1 FOR UPDATE
               )
               UPDATE oauth_auth_codes AS c SET refresh_token_id = NULL
               FROM prior
               WHERE c.code = prior.code
               RETURNING prior.refresh_token_id"#,
            code_hash
        )
        .fetch_optional(self.write_pool_ref())
        .await?
        .flatten();

        let Some(family) = family else {
            tracing::warn!(
                event = "auth_code_replay_detected",
                code_hash = %code_hash,
                "Replayed auth code names no refresh-token family"
            );
            return Ok(());
        };

        let result = sqlx::query!(
            "DELETE FROM oauth_refresh_tokens WHERE family_id = $1",
            family
        )
        .execute(self.write_pool_ref())
        .await?;
        tracing::warn!(
            event = "auth_code_replay_detected",
            code_hash = %code_hash,
            family_id = %family,
            revoked_count = result.rows_affected(),
            "Revoked refresh-token family after auth-code replay"
        );
        Ok(())
    }
}
