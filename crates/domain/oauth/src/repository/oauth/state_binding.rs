//! Server-issued OAuth `state` tokens bound to a stored `return_to`. The raw
//! token leaves the server exactly once (as the `state` query parameter on the
//! authorize redirect); the row is keyed by HMAC-SHA-256 under the deployment
//! pepper, mirroring `auth_code` and refresh-token storage. `consume` is a
//! single atomic UPDATE — re-use, expiry, and tamper attempts all surface as
//! `None`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OAuthRepository;
use super::at_rest::hash_at_rest;
use crate::error::OauthResult;
use chrono::{DateTime, Duration, Utc};
use std::sync::LazyLock;
use systemprompt_identifiers::ClientId;

const DEFAULT_TTL: Duration = Duration::minutes(10);

static EMPTY_CLIENT_ID: LazyLock<ClientId> = LazyLock::new(|| ClientId::new(""));

#[derive(Debug)]
pub struct StateBindingParams<'a> {
    pub state_token: &'a str,
    pub return_to: &'a str,
    pub client_id: &'a ClientId,
    pub redirect_uri: &'a str,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct StateBindingParamsBuilder<'a> {
    state_token: &'a str,
    return_to: Option<&'a str>,
    client_id: Option<&'a ClientId>,
    redirect_uri: Option<&'a str>,
    expires_at: Option<DateTime<Utc>>,
}

impl<'a> StateBindingParamsBuilder<'a> {
    pub const fn new(state_token: &'a str) -> Self {
        Self {
            state_token,
            return_to: None,
            client_id: None,
            redirect_uri: None,
            expires_at: None,
        }
    }

    pub const fn with_return_to(mut self, return_to: &'a str) -> Self {
        self.return_to = Some(return_to);
        self
    }

    pub const fn with_client_id(mut self, client_id: &'a ClientId) -> Self {
        self.client_id = Some(client_id);
        self
    }

    pub const fn with_redirect_uri(mut self, redirect_uri: &'a str) -> Self {
        self.redirect_uri = Some(redirect_uri);
        self
    }

    pub const fn with_expires_at(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    pub fn build(self) -> StateBindingParams<'a> {
        StateBindingParams {
            state_token: self.state_token,
            return_to: self.return_to.unwrap_or("/"),
            client_id: self.client_id.unwrap_or(&EMPTY_CLIENT_ID),
            redirect_uri: self.redirect_uri.unwrap_or(""),
            expires_at: self.expires_at.unwrap_or_else(|| Utc::now() + DEFAULT_TTL),
        }
    }
}

impl<'a> StateBindingParams<'a> {
    pub const fn builder(state_token: &'a str) -> StateBindingParamsBuilder<'a> {
        StateBindingParamsBuilder::new(state_token)
    }
}

#[derive(Debug, Clone)]
pub struct StateBindingRow {
    pub return_to: String,
    pub client_id: ClientId,
    pub redirect_uri: String,
}

impl OAuthRepository {
    pub async fn store_state_binding(&self, params: StateBindingParams<'_>) -> OauthResult<()> {
        let state_token_hash = hash_at_rest(params.state_token)?;
        sqlx::query!(
            "INSERT INTO oauth_state_bindings
             (state_token_hash, return_to, client_id, redirect_uri, created_at, expires_at)
             VALUES ($1, $2, $3, $4, now(), $5)",
            state_token_hash,
            params.return_to,
            params.client_id.as_str(),
            params.redirect_uri,
            params.expires_at,
        )
        .execute(self.write_pool_ref())
        .await?;
        Ok(())
    }

    pub async fn consume_state_binding(
        &self,
        state_token: &str,
    ) -> OauthResult<Option<StateBindingRow>> {
        let state_token_hash = hash_at_rest(state_token)?;
        let row = sqlx::query!(
            "UPDATE oauth_state_bindings
                SET consumed_at = now()
              WHERE state_token_hash = $1
                AND consumed_at IS NULL
                AND expires_at > now()
              RETURNING return_to, client_id, redirect_uri",
            state_token_hash,
        )
        .fetch_optional(self.write_pool_ref())
        .await?;

        Ok(row.map(|r| StateBindingRow {
            return_to: r.return_to,
            client_id: ClientId::new(&r.client_id),
            redirect_uri: r.redirect_uri,
        }))
    }

    pub async fn cleanup_expired_state_bindings(&self) -> OauthResult<u64> {
        let result = sqlx::query!("DELETE FROM oauth_state_bindings WHERE expires_at < now()")
            .execute(self.write_pool_ref())
            .await?;
        Ok(result.rows_affected())
    }
}
