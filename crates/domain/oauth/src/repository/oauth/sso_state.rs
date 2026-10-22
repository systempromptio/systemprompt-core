//! Server-side correlation for browser-redirect SSO. `store` remembers what
//! `finish` needs under the hash of the `state` the identity provider echoes
//! back, together with the hash of the binder cookie that ties the return to
//! the browser that started it. `consume` is a single atomic UPDATE —
//! re-use, expiry and tamper attempts all surface as `None`.
//!
//! Queries are built with `sqlx::query` rather than the checked macros so the
//! table needs no offline query cache entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use subtle::ConstantTimeEq;
use systemprompt_identifiers::SsoProviderId;

use super::OAuthRepository;
use super::at_rest::hash_at_rest;
use crate::error::OauthResult;

#[derive(Debug, Clone, Copy)]
pub struct SsoStateParams<'a> {
    pub state: &'a str,
    pub provider_id: &'a str,
    pub return_to: &'a str,
    pub binder: &'a str,
    pub request_id: Option<&'a str>,
    pub nonce: Option<&'a str>,
    pub pkce_verifier: Option<&'a str>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SsoStateRow {
    pub provider_id: SsoProviderId,
    pub return_to: String,
    pub binder_hash: String,
    pub request_id: Option<String>,
    pub nonce: Option<String>,
    pub pkce_verifier: Option<String>,
}

impl SsoStateRow {
    pub fn binder_matches(&self, binder: &str) -> OauthResult<bool> {
        let presented = hash_at_rest(binder)?;
        Ok(bool::from(
            presented.as_bytes().ct_eq(self.binder_hash.as_bytes()),
        ))
    }
}

impl OAuthRepository {
    pub async fn store_sso_state(&self, params: &SsoStateParams<'_>) -> OauthResult<()> {
        let state_hash = hash_at_rest(params.state)?;
        let binder_hash = hash_at_rest(params.binder)?;
        sqlx::query!(
            "INSERT INTO sso_state_bindings
             (state_hash, provider_id, return_to, binder_hash, request_id, nonce,
              pkce_verifier, created_at, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, now(), $8)",
            state_hash,
            params.provider_id,
            params.return_to,
            binder_hash,
            params.request_id,
            params.nonce,
            params.pkce_verifier,
            params.expires_at
        )
        .execute(self.write_pool_ref())
        .await?;
        Ok(())
    }

    pub async fn consume_sso_state(
        &self,
        provider_id: &str,
        state: &str,
    ) -> OauthResult<Option<SsoStateRow>> {
        let state_hash = hash_at_rest(state)?;
        let row = sqlx::query_as!(
            SsoStateRow,
            "UPDATE sso_state_bindings
                SET consumed_at = now()
              WHERE state_hash = $1
                AND provider_id = $2
                AND consumed_at IS NULL
                AND expires_at > now()
              RETURNING provider_id, return_to, binder_hash, request_id, nonce, pkce_verifier",
            state_hash,
            provider_id
        )
        .fetch_optional(self.write_pool_ref())
        .await?;
        Ok(row)
    }
}
