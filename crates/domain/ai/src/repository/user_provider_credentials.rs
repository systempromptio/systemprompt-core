//! Repository for `user_provider_credentials`: provider keys a user stored for
//! themselves (personal subscriptions).
//!
//! The key is sealed with [`systemprompt_security::at_rest`] before it is
//! written and opened only by [`UserProviderCredentialRepository::find_key`],
//! the gateway's dispatch path. Every other read returns metadata alone, so no
//! listing, log or API response can carry a stored key.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_traits::RepositoryError;

/// What is known about a stored key without opening it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UserProviderCredentialMeta {
    pub provider: String,
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct UserProviderCredentialRepository {
    write_pool: Arc<PgPool>,
}

const NONCE_HEX_CHARS: usize = 24;

impl UserProviderCredentialRepository {
    pub fn new(db: &DbPool) -> Self {
        let write_pool = db.write_pool();
        Self { write_pool }
    }

    /// Seals and stores `key` as the user's credential for `provider`,
    /// replacing any earlier one; the caller has already verified it upstream.
    pub async fn set_key(
        &self,
        user_id: &UserId,
        provider: &str,
        key: &str,
        label: Option<&str>,
    ) -> Result<UserProviderCredentialMeta, RepositoryError> {
        let sealed = systemprompt_security::at_rest::seal(key).map_err(|e| {
            RepositoryError::invalid_argument("encryption_master_key", e.to_string())
        })?;
        let nonce = sealed.get(..NONCE_HEX_CHARS).unwrap_or_default();
        let row = sqlx::query_as!(
            UserProviderCredentialMeta,
            r#"
            INSERT INTO user_provider_credentials
                (user_id, provider, ciphertext, nonce, label, verified_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            ON CONFLICT (user_id, provider) DO UPDATE SET
                ciphertext = EXCLUDED.ciphertext,
                nonce = EXCLUDED.nonce,
                label = EXCLUDED.label,
                created_at = NOW(),
                verified_at = NOW(),
                last_used_at = NULL
            RETURNING provider, label, created_at, verified_at, last_used_at
            "#,
            user_id.as_str(),
            provider,
            sealed.as_str(),
            nonce,
            label,
        )
        .fetch_one(&*self.write_pool)
        .await?;
        Ok(row)
    }

    /// Opens the user's key for `provider` and stamps it used. A row that no
    /// longer opens (the master key rolled) reads as absent, so the caller is
    /// asked to store the key again rather than sent upstream with garbage.
    pub async fn find_key(
        &self,
        user_id: &UserId,
        provider: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let sealed = sqlx::query_scalar!(
            r#"
            UPDATE user_provider_credentials
            SET last_used_at = NOW()
            WHERE user_id = $1 AND provider = $2
            RETURNING ciphertext
            "#,
            user_id.as_str(),
            provider,
        )
        .fetch_optional(&*self.write_pool)
        .await?;
        let Some(sealed) = sealed else {
            return Ok(None);
        };
        match systemprompt_security::at_rest::open(&sealed) {
            Ok(key) => Ok(Some(key)),
            Err(error) => {
                tracing::warn!(
                    user_id = %user_id,
                    provider,
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
            SELECT provider, label, created_at, verified_at, last_used_at
            FROM user_provider_credentials
            WHERE user_id = $1
            ORDER BY provider
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
        provider: &str,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            r#"DELETE FROM user_provider_credentials WHERE user_id = $1 AND provider = $2"#,
            user_id.as_str(),
            provider,
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
