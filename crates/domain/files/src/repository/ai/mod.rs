use anyhow::{Context, Result};
use systemprompt_identifiers::{SessionId, TraceId, UserId};

use super::file::FileRepository;
use crate::models::File;

impl FileRepository {
    pub async fn list_ai_images(&self, limit: i64, offset: i64) -> Result<Vec<File>> {
        sqlx::query_as!(
            File,
            r#"
            SELECT id, path, public_url, mime_type, size_bytes, ai_content, metadata, user_id as "user_id: UserId", session_id as "session_id: SessionId", trace_id as "trace_id: TraceId", created_at, updated_at, deleted_at
            FROM files
            WHERE ai_content = true AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(self.pool.as_ref())
        .await
        .context("Failed to list AI images")
    }

    pub async fn list_ai_images_by_user(
        &self,
        user_id: &UserId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<File>> {
        let user_id_str = user_id.as_str();
        sqlx::query_as!(
            File,
            r#"
            SELECT id, path, public_url, mime_type, size_bytes, ai_content, metadata, user_id as "user_id: UserId", session_id as "session_id: SessionId", trace_id as "trace_id: TraceId", created_at, updated_at, deleted_at
            FROM files
            WHERE user_id = $1 AND ai_content = true AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            user_id_str,
            limit,
            offset
        )
        .fetch_all(self.pool.as_ref())
        .await
        .context(format!("Failed to list AI images for user: {user_id}"))
    }

    pub async fn count_ai_images_by_user(&self, user_id: &UserId) -> Result<i64> {
        let user_id_str = user_id.as_str();
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM files
            WHERE user_id = $1 AND ai_content = true AND deleted_at IS NULL
            "#,
            user_id_str
        )
        .fetch_one(self.pool.as_ref())
        .await
        .context(format!("Failed to count AI images for user: {user_id}"))?;

        Ok(count)
    }
}
