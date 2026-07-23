//! Authorisation code persistence with PKCE. The `code` and the linked
//! `refresh_token_id` are stored as HMAC-SHA-256 digests under the deployment
//! pepper; raw values never touch the database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OAuthRepository;
use super::at_rest::hash_at_rest;
use crate::error::{OauthError, OauthResult};
use crate::models::PkceMethod;
use base64::Engine;
use chrono::Utc;
use subtle::ConstantTimeEq;
use systemprompt_identifiers::{AuthorizationCode, ClientId, UserId};

mod params;

pub use params::{AuthCodeParams, AuthCodeValidationResult};

impl OAuthRepository {
    pub async fn store_authorization_code(&self, params: AuthCodeParams<'_>) -> OauthResult<()> {
        let expires_at = Utc::now() + chrono::Duration::seconds(600);
        let now = Utc::now();
        let code_hash = hash_at_rest(params.code.as_str())?;
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
            params.code_challenge_method,
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
        let code_hash = hash_at_rest(code.as_str())?;
        let result = sqlx::query_scalar!(
            "SELECT client_id FROM oauth_auth_codes WHERE code = $1",
            code_hash
        )
        .fetch_optional(self.pool_ref())
        .await?;

        Ok(result.map(ClientId::new))
    }

    pub async fn validate_authorization_code(
        &self,
        code: &AuthorizationCode,
        client_id: &ClientId,
        redirect_uri: Option<&str>,
        code_verifier: Option<&str>,
    ) -> OauthResult<AuthCodeValidationResult> {
        let now = Utc::now();
        let code_hash = hash_at_rest(code.as_str())?;

        // Why: Atomically claim the code: only one concurrent caller wins the
        // `used_at IS NULL` race. The application-level checks below run
        // against the row the winner just locked, so two simultaneous
        // exchanges of the same code cannot both pass validation.
        let claimed = sqlx::query!(
            "UPDATE oauth_auth_codes
             SET used_at = $1
             WHERE code = $2 AND used_at IS NULL
             RETURNING client_id, user_id, scope, expires_at, redirect_uri, code_challenge,
                       code_challenge_method, resource",
            now,
            code_hash
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

        if let Some(expected_uri) = redirect_uri
            && row.redirect_uri != expected_uri
        {
            tracing::warn!(
                expected = %expected_uri,
                actual = %row.redirect_uri,
                "Redirect URI mismatch"
            );
            return Err(OauthError::Validation(
                "Invalid authorization code".to_owned(),
            ));
        }

        if let Some(ref challenge) = row.code_challenge {
            verify_pkce(
                challenge,
                row.code_challenge_method.as_deref(),
                code_verifier,
            )?;
        }

        Ok(AuthCodeValidationResult {
            user_id: UserId::new(row.user_id),
            scope: row.scope,
            resource: row.resource,
        })
    }

    // Why: Disambiguate "atomic claim returned no row." Either the code never
    // existed, or it was previously consumed (replay) — the latter must
    // revoke the entire refresh-token family per RFC 6819 §5.2.2.3.
    async fn handle_unclaimable_auth_code(
        &self,
        code_hash: &str,
    ) -> OauthResult<AuthCodeValidationResult> {
        let existing = sqlx::query!(
            "SELECT used_at, refresh_token_id FROM oauth_auth_codes WHERE code = $1",
            code_hash
        )
        .fetch_optional(self.pool_ref())
        .await?;

        let Some(row) = existing else {
            tracing::warn!("Authorization code not found");
            return Err(OauthError::Validation(
                "Invalid authorization code".to_owned(),
            ));
        };

        if row.used_at.is_some() {
            tracing::warn!(
                code_hash = %code_hash,
                "Authorization code replay detected"
            );

            if let Some(rt_id) = row.refresh_token_id {
                let family_id = sqlx::query_scalar!(
                    "SELECT family_id FROM oauth_refresh_tokens WHERE token_id = $1",
                    rt_id
                )
                .fetch_optional(self.pool_ref())
                .await?;
                if let Some(family) = family_id {
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
                } else {
                    tracing::warn!(
                        event = "auth_code_replay_detected",
                        code_hash = %code_hash,
                        "Linked refresh token already gone; no family to revoke"
                    );
                }
            } else {
                tracing::warn!(
                    event = "auth_code_replay_detected",
                    code_hash = %code_hash,
                    "No refresh token linked to replayed auth code"
                );
            }
        }

        Err(OauthError::Validation(
            "Invalid authorization code".to_owned(),
        ))
    }

    pub async fn link_auth_code_to_refresh_token(
        &self,
        code: &AuthorizationCode,
        refresh_token_id: &str,
    ) -> OauthResult<()> {
        let code_hash = hash_at_rest(code.as_str())?;
        let refresh_token_id_hash = hash_at_rest(refresh_token_id)?;
        sqlx::query!(
            "UPDATE oauth_auth_codes SET refresh_token_id = $1 WHERE code = $2",
            refresh_token_id_hash,
            code_hash
        )
        .execute(self.write_pool_ref())
        .await?;
        Ok(())
    }
}

fn verify_pkce(
    challenge: &str,
    method: Option<&str>,
    code_verifier: Option<&str>,
) -> OauthResult<()> {
    let verifier = code_verifier.ok_or_else(|| {
        tracing::warn!("Missing code_verifier for PKCE challenge");
        OauthError::Validation("Invalid authorization code".to_owned())
    })?;

    let method = method.ok_or_else(|| {
        tracing::warn!("Missing code_challenge_method for PKCE challenge");
        OauthError::Validation("Invalid authorization code".to_owned())
    })?;

    let computed_challenge = match method.parse::<PkceMethod>() {
        Ok(PkceMethod::S256) => {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(verifier.as_bytes());
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
        },
        Err(e) => {
            tracing::warn!(method = %method, error = %e, "Unsupported code_challenge_method");
            return Err(OauthError::Validation(
                "Invalid authorization code".to_owned(),
            ));
        },
    };

    let challenge_matches: bool = computed_challenge
        .as_bytes()
        .ct_eq(challenge.as_bytes())
        .into();
    if challenge_matches {
        Ok(())
    } else {
        tracing::warn!("PKCE validation failed");
        Err(OauthError::Validation(
            "Invalid authorization code".to_owned(),
        ))
    }
}
