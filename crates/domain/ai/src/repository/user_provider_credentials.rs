//! Repository for `user_provider_credentials`: keys a member stored for
//! themselves (personal subscriptions), one per catalog credential id.
//!
//! The key arrives sealed with [`systemprompt_security::at_rest`]
//! (`crate::store_personal_key` seals it) and is opened only by
//! [`UserProviderCredentialRepository::find_key`],
//! the gateway's dispatch path. Every other read returns metadata alone, so no
//! listing, log or API response can carry a stored key.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CredentialId, UserId};
use systemprompt_security::at_rest::AtRestCipher;
use systemprompt_traits::RepositoryError;

/// What is known about a stored key without opening it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UserProviderCredentialMeta {
    pub credential: String,
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct UserProviderCredentialRepository {
    write_pool: Arc<PgPool>,
    cipher: AtRestCipher,
}

const NONCE_HEX_CHARS: usize = 24;

impl UserProviderCredentialRepository {
    pub fn new(db: &DbPool, cipher: AtRestCipher) -> Self {
        let write_pool = db.write_pool();
        Self { write_pool, cipher }
    }

    pub const fn cipher(&self) -> &AtRestCipher {
        &self.cipher
    }

    pub async fn set_key(
        &self,
        user_id: &UserId,
        credential: &CredentialId,
        sealed: &str,
        label: Option<&str>,
    ) -> Result<UserProviderCredentialMeta, RepositoryError> {
        let nonce = sealed.get(..NONCE_HEX_CHARS).unwrap_or_default();
        let row = sqlx::query_as!(
            UserProviderCredentialMeta,
            r#"
            INSERT INTO user_provider_credentials
                (user_id, credential, ciphertext, nonce, label, verified_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            ON CONFLICT (user_id, credential) DO UPDATE SET
                ciphertext = EXCLUDED.ciphertext,
                nonce = EXCLUDED.nonce,
                label = EXCLUDED.label,
                created_at = NOW(),
                verified_at = NOW(),
                last_used_at = NULL
            RETURNING credential, label, created_at, verified_at, last_used_at
            "#,
            user_id.as_str(),
            credential.as_str(),
            sealed,
            nonce,
            label,
        )
        .fetch_one(&*self.write_pool)
        .await?;
        Ok(row)
    }

    pub async fn find_key(
        &self,
        user_id: &UserId,
        credential: &CredentialId,
    ) -> Result<Option<String>, RepositoryError> {
        let sealed = sqlx::query_scalar!(
            r#"
            UPDATE user_provider_credentials
            SET last_used_at = NOW()
            WHERE user_id = $1 AND credential = $2
            RETURNING ciphertext
            "#,
            user_id.as_str(),
            credential.as_str(),
        )
        .fetch_optional(&*self.write_pool)
        .await?;
        let Some(sealed) = sealed else {
            return Ok(None);
        };
        match self.cipher.open(&sealed) {
            Ok(key) => Ok(Some(key)),
            Err(error) => {
                tracing::warn!(
                    user_id = %user_id,
                    credential = %credential.as_str(),
                    %error,
                    "Stored provider credential did not open; treating it as absent"
                );
                Ok(None)
            },
        }
    }

    pub async fn list_metadata(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<UserProviderCredentialMeta>, RepositoryError> {
        let rows = sqlx::query_as!(
            UserProviderCredentialMeta,
            r#"
            SELECT credential, label, created_at, verified_at, last_used_at
            FROM user_provider_credentials
            WHERE user_id = $1
            ORDER BY credential
            "#,
            user_id.as_str(),
        )
        .fetch_all(&*self.write_pool)
        .await?;
        Ok(rows)
    }

    pub async fn delete_key(
        &self,
        user_id: &UserId,
        credential: &CredentialId,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            r#"DELETE FROM user_provider_credentials WHERE user_id = $1 AND credential = $2"#,
            user_id.as_str(),
            credential.as_str(),
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
