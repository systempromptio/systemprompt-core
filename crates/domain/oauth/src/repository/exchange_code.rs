//! Single-use bridge exchange code persistence.
//!
//! A device-link code may carry the UI grant the browser approved. Only
//! `consume_device_link_code` hands that grant back, so the session exchange,
//! which redeems the same codes for a bridge token, never yields one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::OauthResult as Result;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{ClientId, UserId};

use crate::repository::OAuthRepository;

#[derive(Debug)]
pub struct CreateExchangeCodeParams<'a> {
    pub code_hash: &'a str,
    pub user_id: &'a UserId,
    pub expires_at: DateTime<Utc>,
}

/// The person's UI session a device link was approved for.
///
/// Names the `person_ui` client, its redirect, the scope, the S256 challenge
/// whose verifier stays with the client, and when the person signed in to the
/// approving browser.
#[derive(Debug, Clone, Copy)]
pub struct DeviceLinkUiGrant<'a> {
    pub client_id: &'a ClientId,
    pub redirect_uri: &'a str,
    pub scope: &'a str,
    pub code_challenge: &'a str,
    pub authenticated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredUiGrant {
    pub client_id: ClientId,
    pub redirect_uri: String,
    pub scope: String,
    pub code_challenge: String,
    pub authenticated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumedDeviceLink {
    pub user_id: UserId,
    pub ui_grant: Option<StoredUiGrant>,
}

impl OAuthRepository {
    pub async fn create_device_link_code(
        &self,
        params: CreateExchangeCodeParams<'_>,
        grant: DeviceLinkUiGrant<'_>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO bridge_exchange_codes
                (code_hash, user_id, expires_at, ui_client_id, ui_redirect_uri, ui_scope,
                 ui_code_challenge, ui_authenticated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            params.code_hash,
            params.user_id as _,
            params.expires_at,
            grant.client_id as _,
            grant.redirect_uri,
            grant.scope,
            grant.code_challenge,
            grant.authenticated_at,
        )
        .execute(self.write_pool_ref())
        .await?;
        Ok(())
    }

    pub async fn consume_device_link_code(
        &self,
        code_hash: &str,
    ) -> Result<Option<ConsumedDeviceLink>> {
        let row = sqlx::query!(
            r#"
            UPDATE bridge_exchange_codes
            SET consumed_at = CURRENT_TIMESTAMP
            WHERE code_hash = $1
              AND consumed_at IS NULL
              AND expires_at > CURRENT_TIMESTAMP
            RETURNING user_id, ui_client_id, ui_redirect_uri, ui_scope, ui_code_challenge,
                      ui_authenticated_at
            "#,
            code_hash,
        )
        .fetch_optional(self.write_pool_ref())
        .await?;
        Ok(row.map(|r| {
            let ui_grant = match (
                r.ui_client_id,
                r.ui_redirect_uri,
                r.ui_scope,
                r.ui_code_challenge,
                r.ui_authenticated_at,
            ) {
                (
                    Some(client_id),
                    Some(redirect_uri),
                    Some(scope),
                    Some(code_challenge),
                    Some(authenticated_at),
                ) => Some(StoredUiGrant {
                    client_id: ClientId::new(client_id),
                    redirect_uri,
                    scope,
                    code_challenge,
                    authenticated_at,
                }),
                _ => None,
            };
            ConsumedDeviceLink {
                user_id: UserId::new(r.user_id),
                ui_grant,
            }
        }))
    }

    pub async fn create_bridge_exchange_code(
        &self,
        params: CreateExchangeCodeParams<'_>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO bridge_exchange_codes (code_hash, user_id, expires_at)
            VALUES ($1, $2, $3)
            "#,
            params.code_hash,
            params.user_id as _,
            params.expires_at,
        )
        .execute(self.write_pool_ref())
        .await?;
        Ok(())
    }

    pub async fn consume_bridge_exchange_code(&self, code_hash: &str) -> Result<Option<UserId>> {
        let row = sqlx::query!(
            r#"
            UPDATE bridge_exchange_codes
            SET consumed_at = CURRENT_TIMESTAMP
            WHERE code_hash = $1
              AND consumed_at IS NULL
              AND expires_at > CURRENT_TIMESTAMP
            RETURNING user_id
            "#,
            code_hash,
        )
        .fetch_optional(self.write_pool_ref())
        .await?;
        Ok(row.map(|r| UserId::new(r.user_id)))
    }
}
