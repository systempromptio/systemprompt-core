use chrono::{DateTime, Utc};
use systemprompt_identifiers::{ApiKeyId, UserId};

use crate::error::Result;
use crate::models::UserApiKey;
use crate::repository::UserRepository;

pub struct CreateApiKeyParams<'a> {
    pub id: &'a ApiKeyId,
    pub user_id: &'a UserId,
    pub name: &'a str,
    pub key_prefix: &'a str,
    pub key_hash: &'a str,
    pub expires_at: Option<DateTime<Utc>>,
}

impl UserRepository {
    pub async fn create_api_key(&self, params: CreateApiKeyParams<'_>) -> Result<UserApiKey> {
        let row = sqlx::query_as!(
            UserApiKey,
            r#"
            INSERT INTO user_api_keys
                (id, user_id, name, key_prefix, key_hash, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, user_id, name, key_prefix, key_hash,
                      created_at, last_used_at, expires_at, revoked_at
            "#,
            params.id.as_str(),
            params.user_id.as_str(),
            params.name,
            params.key_prefix,
            params.key_hash,
            params.expires_at,
        )
        .fetch_one(&*self.write_pool)
        .await?;
        Ok(row)
    }

    pub async fn find_active_api_key_by_prefix(
        &self,
        key_prefix: &str,
    ) -> Result<Option<UserApiKey>> {
        let row = sqlx::query_as!(
            UserApiKey,
            r#"
            SELECT id, user_id, name, key_prefix, key_hash,
                   created_at, last_used_at, expires_at, revoked_at
            FROM user_api_keys
            WHERE key_prefix = $1
              AND revoked_at IS NULL
            "#,
            key_prefix,
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_api_keys_for_user(&self, user_id: &UserId) -> Result<Vec<UserApiKey>> {
        let rows = sqlx::query_as!(
            UserApiKey,
            r#"
            SELECT id, user_id, name, key_prefix, key_hash,
                   created_at, last_used_at, expires_at, revoked_at
            FROM user_api_keys
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
            user_id.as_str(),
        )
        .fetch_all(&*self.pool)
        .await?;
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
