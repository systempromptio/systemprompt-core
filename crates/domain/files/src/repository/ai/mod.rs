//! [`FileRepository`] queries for AI-generated files.
//!
//! Listing and counting of files flagged with `ai_content`, both globally and
//! scoped to an owning user.

use systemprompt_identifiers::{ContextId, SessionId, TraceId, UserId};

use super::file::FileRepository;
use crate::error::FilesResult;
use crate::models::{File, FileMetadata};

impl FileRepository {
    pub async fn list_ai_images(&self, limit: i64, offset: i64) -> FilesResult<Vec<File>> {
        let result = sqlx::query_as!(
            File,
            r#"
            SELECT id, path, public_url, mime_type, size_bytes, ai_content, metadata as "metadata: sqlx::types::Json<FileMetadata>", user_id as "user_id: UserId", session_id as "session_id: SessionId", trace_id as "trace_id: TraceId", context_id as "context_id: ContextId", created_at, updated_at, deleted_at
            FROM files
            WHERE ai_content = true AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(result)
    }

    pub async fn list_ai_images_by_user(
        &self,
        user_id: &UserId,
        limit: i64,
        offset: i64,
    ) -> FilesResult<Vec<File>> {
        let user_id_str = user_id.as_str();
        let result = sqlx::query_as!(
            File,
            r#"
            SELECT id, path, public_url, mime_type, size_bytes, ai_content, metadata as "metadata: sqlx::types::Json<FileMetadata>", user_id as "user_id: UserId", session_id as "session_id: SessionId", trace_id as "trace_id: TraceId", context_id as "context_id: ContextId", created_at, updated_at, deleted_at
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
        .await?;

        Ok(result)
    }

    pub async fn count_ai_images_by_user(&self, user_id: &UserId) -> FilesResult<i64> {
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
        .await?;

        Ok(count)
    }

    pub async fn count_ai_images(&self) -> FilesResult<i64> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) as "count!"
            FROM files
            WHERE ai_content = true AND deleted_at IS NULL
            "#
        )
        .fetch_one(self.pool.as_ref())
        .await?;

        Ok(count)
    }
}
