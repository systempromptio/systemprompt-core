//! `WebAuthn` credential persistence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{OauthError, OauthResult as Result};
use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;

#[derive(Debug, Clone)]
pub struct WebAuthnCredential {
    pub id: String,
    pub user_id: UserId,
    pub credential_id: Vec<u8>,
    pub public_key: Vec<u8>,
    pub display_name: String,
    pub device_type: String,
    pub transports: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct WebAuthnCredentialParams<'a> {
    pub id: &'a str,
    pub user_id: &'a UserId,
    pub credential_id: &'a [u8],
    pub public_key: &'a [u8],
    pub display_name: &'a str,
    pub device_type: &'a str,
    pub transports: &'a [String],
}

#[derive(Debug)]
pub struct WebAuthnCredentialParamsBuilder<'a> {
    id: &'a str,
    user_id: &'a UserId,
    credential_id: &'a [u8],
    public_key: &'a [u8],
    display_name: &'a str,
    device_type: &'a str,
    transports: &'a [String],
}

impl<'a> WebAuthnCredentialParamsBuilder<'a> {
    pub const fn new(
        id: &'a str,
        user_id: &'a UserId,
        credential_id: &'a [u8],
        public_key: &'a [u8],
    ) -> Self {
        Self {
            id,
            user_id,
            credential_id,
            public_key,
            display_name: "",
            device_type: "",
            transports: &[],
        }
    }

    pub const fn with_display_name(mut self, display_name: &'a str) -> Self {
        self.display_name = display_name;
        self
    }

    pub const fn with_device_type(mut self, device_type: &'a str) -> Self {
        self.device_type = device_type;
        self
    }

    pub const fn with_transports(mut self, transports: &'a [String]) -> Self {
        self.transports = transports;
        self
    }

    pub const fn build(self) -> WebAuthnCredentialParams<'a> {
        WebAuthnCredentialParams {
            id: self.id,
            user_id: self.user_id,
            credential_id: self.credential_id,
            public_key: self.public_key,
            display_name: self.display_name,
            device_type: self.device_type,
            transports: self.transports,
        }
    }
}

impl<'a> WebAuthnCredentialParams<'a> {
    pub const fn builder(
        id: &'a str,
        user_id: &'a UserId,
        credential_id: &'a [u8],
        public_key: &'a [u8],
    ) -> WebAuthnCredentialParamsBuilder<'a> {
        WebAuthnCredentialParamsBuilder::new(id, user_id, credential_id, public_key)
    }
}

impl crate::repository::OAuthRepository {
    pub async fn store_webauthn_credential(
        &self,
        params: WebAuthnCredentialParams<'_>,
    ) -> Result<()> {
        let transports_json = serde_json::to_string(params.transports)?;
        let now = Utc::now();

        sqlx::query!(
            "INSERT INTO webauthn_credentials
             (id, user_id, credential_id, public_key, display_name, device_type, transports,
             created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            params.id,
            params.user_id.as_str(),
            params.credential_id,
            params.public_key,
            params.display_name,
            params.device_type,
            transports_json,
            now
        )
        .execute(self.write_pool_ref())
        .await?;

        Ok(())
    }

    pub async fn list_webauthn_credentials(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<WebAuthnCredential>> {
        let user_id_str = user_id.as_str();
        let rows = sqlx::query!(
            "SELECT id, user_id, credential_id, public_key, display_name, device_type,
                    transports, created_at, last_used_at
             FROM webauthn_credentials WHERE user_id = $1 ORDER BY created_at DESC",
            user_id_str
        )
        .fetch_all(self.write_pool_ref())
        .await?;

        rows.into_iter()
            .map(|row| {
                let transports: Vec<String> = serde_json::from_str(&row.transports)?;
                Ok(WebAuthnCredential {
                    id: row.id,
                    user_id: UserId::new(row.user_id),
                    credential_id: row.credential_id,
                    public_key: row.public_key,
                    display_name: row.display_name,
                    device_type: row.device_type,
                    transports,
                    created_at: row.created_at,
                    last_used_at: row.last_used_at,
                })
            })
            .collect()
    }

    pub async fn replace_webauthn_passkey(
        &self,
        credential_id: &[u8],
        previous_passkey: &[u8],
        updated_passkey: &[u8],
    ) -> Result<()> {
        let now = Utc::now();
        let result = sqlx::query!(
            "UPDATE webauthn_credentials SET public_key = $1, last_used_at = $2
             WHERE credential_id = $3 AND public_key = $4",
            updated_passkey,
            now,
            credential_id,
            previous_passkey
        )
        .execute(self.write_pool_ref())
        .await?;

        if result.rows_affected() == 0 {
            return Err(OauthError::WebAuthnVerificationFailed(
                "stored passkey changed during authentication".to_owned(),
            ));
        }
        Ok(())
    }

    pub async fn touch_webauthn_credential(&self, credential_id: &[u8]) -> Result<()> {
        let now = Utc::now();
        let result = sqlx::query!(
            "UPDATE webauthn_credentials SET last_used_at = $1 WHERE credential_id = $2",
            now,
            credential_id
        )
        .execute(self.write_pool_ref())
        .await?;

        if result.rows_affected() == 0 {
            return Err(OauthError::WebAuthnVerificationFailed(
                "credential not found".to_owned(),
            ));
        }
        Ok(())
    }
}
