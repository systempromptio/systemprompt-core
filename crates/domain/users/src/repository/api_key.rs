//! API-key persistence on the user repository.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{ApiKeyId, UserId};
use systemprompt_models::attribution::ScopeBinding;

use crate::error::Result;
use crate::models::{ApiKeyLimits, UserApiKey, UserApiKeyRow};
use crate::repository::UserRepository;

#[derive(Debug)]
pub struct CreateApiKeyParams<'a> {
    pub id: &'a ApiKeyId,
    pub user_id: &'a UserId,
    pub name: &'a str,
    pub key_prefix: &'a str,
    pub key_hash: &'a str,
    pub expires_at: Option<DateTime<Utc>>,
    pub limits: &'a ApiKeyLimits,
    pub scopes: &'a [ScopeBinding],
}

impl UserRepository {
    pub async fn create_api_key(&self, params: CreateApiKeyParams<'_>) -> Result<UserApiKey> {
        let mut tx = self.write_pool.begin().await?;
        let row = sqlx::query_as!(
            UserApiKeyRow,
            r#"
            INSERT INTO user_api_keys
                (id, user_id, name, key_prefix, key_hash, expires_at,
                 model_allowlist, budget_microdollars, max_requests, request_window_seconds)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, user_id, name, key_prefix, key_hash,
                      created_at, last_used_at, expires_at, revoked_at,
                      model_allowlist, budget_microdollars, max_requests, request_window_seconds,
                      ARRAY[]::TEXT[] AS "scope_dimensions!", ARRAY[]::TEXT[] AS "scope_values!"
            "#,
            params.id.as_str(),
            params.user_id.as_str(),
            params.name,
            params.key_prefix,
            params.key_hash,
            params.expires_at,
            params.limits.model_allowlist.as_deref(),
            params.limits.budget_microdollars,
            params.limits.max_requests,
            params.limits.request_window_seconds,
        )
        .fetch_one(&mut *tx)
        .await?;
        if !params.scopes.is_empty() {
            let dimensions: Vec<String> = params
                .scopes
                .iter()
                .map(|s| s.dimension.as_str().to_owned())
                .collect();
            let values: Vec<String> = params.scopes.iter().map(|s| s.value.clone()).collect();
            sqlx::query!(
                r#"
                INSERT INTO user_api_key_scopes (key_id, dimension, value)
                SELECT $1, t.dimension, t.value
                FROM UNNEST($2::text[], $3::text[]) AS t(dimension, value)
                "#,
                params.id.as_str(),
                &dimensions,
                &values,
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        let mut key = UserApiKey::from(row);
        key.scopes = params.scopes.to_vec();
        Ok(key)
    }

    pub async fn find_active_api_key_by_prefix(
        &self,
        key_prefix: &str,
    ) -> Result<Option<UserApiKey>> {
        let row = sqlx::query_as!(
            UserApiKeyRow,
            r#"
            SELECT k.id, k.user_id, k.name, k.key_prefix, k.key_hash,
                   k.created_at, k.last_used_at, k.expires_at, k.revoked_at,
                   k.model_allowlist, k.budget_microdollars, k.max_requests,
                   k.request_window_seconds,
                   ARRAY(SELECT s.dimension FROM user_api_key_scopes s
                         WHERE s.key_id = k.id ORDER BY s.dimension) AS "scope_dimensions!",
                   ARRAY(SELECT s.value FROM user_api_key_scopes s
                         WHERE s.key_id = k.id ORDER BY s.dimension) AS "scope_values!"
            FROM user_api_keys k
            WHERE k.key_prefix = $1
              AND k.revoked_at IS NULL
            "#,
            key_prefix,
        )
        .fetch_optional(&*self.write_pool)
        .await
        .map(|row| row.map(UserApiKey::from))?;
        Ok(row)
    }

    pub async fn list_api_keys_for_user(&self, user_id: &UserId) -> Result<Vec<UserApiKey>> {
        let rows = sqlx::query_as!(
            UserApiKeyRow,
            r#"
            SELECT k.id, k.user_id, k.name, k.key_prefix, k.key_hash,
                   k.created_at, k.last_used_at, k.expires_at, k.revoked_at,
                   k.model_allowlist, k.budget_microdollars, k.max_requests,
                   k.request_window_seconds,
                   ARRAY(SELECT s.dimension FROM user_api_key_scopes s
                         WHERE s.key_id = k.id ORDER BY s.dimension) AS "scope_dimensions!",
                   ARRAY(SELECT s.value FROM user_api_key_scopes s
                         WHERE s.key_id = k.id ORDER BY s.dimension) AS "scope_values!"
            FROM user_api_keys k
            WHERE k.user_id = $1
            ORDER BY k.created_at DESC
            "#,
            user_id.as_str(),
        )
        .fetch_all(&*self.pool)
        .await
        .map(|rows| rows.into_iter().map(UserApiKey::from).collect::<Vec<_>>())?;
        Ok(rows)
    }

    pub async fn revoke_api_key(&self, id: &ApiKeyId, user_id: &UserId) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE user_api_keys
            SET revoked_at = CURRENT_TIMESTAMP
            WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL
            "#,
            id.as_str(),
            user_id.as_str(),
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_revoked_api_key_ids_for_user(&self, user_id: &UserId) -> Result<Vec<String>> {
        let rows = sqlx::query_scalar!(
            r#"
            SELECT id
            FROM user_api_keys
            WHERE user_id = $1 AND revoked_at IS NOT NULL
            ORDER BY revoked_at DESC
            "#,
            user_id.as_str(),
        )
        .fetch_all(&*self.write_pool)
        .await?;
        Ok(rows)
    }

    pub async fn touch_api_key_usage(&self, id: &ApiKeyId) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE user_api_keys
            SET last_used_at = CURRENT_TIMESTAMP
            WHERE id = $1
            "#,
            id.as_str(),
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }
}
