use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::PgPool;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::{FileId, SessionId, TraceId, UserId};

use crate::models::{File, FileMetadata};

#[derive(Debug, Clone)]
pub struct InsertFileRequest {
    pub id: FileId,
    pub path: String,
    pub public_url: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    pub ai_content: bool,
    pub metadata: serde_json::Value,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
}

impl InsertFileRequest {
    pub fn new(
        id: FileId,
        path: impl Into<String>,
        public_url: impl Into<String>,
        mime_type: impl Into<String>,
    ) -> Self {
        Self {
            id,
            path: path.into(),
            public_url: public_url.into(),
            mime_type: mime_type.into(),
            size_bytes: None,
            ai_content: false,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            user_id: None,
            session_id: None,
            trace_id: None,
        }
    }

    pub const fn with_size(mut self, size: i64) -> Self {
        self.size_bytes = Some(size);
        self
    }

    pub const fn with_ai_content(mut self, ai_content: bool) -> Self {
        self.ai_content = ai_content;
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn with_user_id(mut self, user_id: UserId) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = Some(trace_id);
        self
    }
}

#[derive(Debug, Clone)]
pub struct FileRepository {
    pub(crate) pool: Arc<PgPool>,
}

impl FileRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.as_ref().get_postgres_pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn insert(&self, request: InsertFileRequest) -> Result<FileId> {
        let id_uuid = uuid::Uuid::parse_str(request.id.as_str())
            .with_context(|| format!("Invalid UUID for file id: {}", request.id.as_str()))?;
        let now = Utc::now();

        let user_id_str = request.user_id.as_ref().map(UserId::as_str);
        let session_id_str = request.session_id.as_ref().map(SessionId::as_str);
        let trace_id_str = request.trace_id.as_ref().map(TraceId::as_str);

        sqlx::query_as!(
            File,
            r#"
            INSERT INTO files (id, path, public_url, mime_type, size_bytes, ai_content, metadata, user_id, session_id, trace_id, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11)
            ON CONFLICT (path) DO UPDATE SET
                public_url = EXCLUDED.public_url,
                mime_type = EXCLUDED.mime_type,
                size_bytes = EXCLUDED.size_bytes,
                ai_content = EXCLUDED.ai_content,
                metadata = EXCLUDED.metadata,
                updated_at = EXCLUDED.updated_at
            RETURNING id, path, public_url, mime_type, size_bytes, ai_content, metadata, user_id as "user_id: UserId", session_id as "session_id: SessionId", trace_id as "trace_id: TraceId", created_at, updated_at, deleted_at
            "#,
            id_uuid,
            request.path,
            request.public_url,
            request.mime_type,
            request.size_bytes,
            request.ai_content,
            request.metadata,
            user_id_str,
            session_id_str,
            trace_id_str,
            now
        )
        .fetch_one(self.pool.as_ref())
        .await
        .with_context(|| {
            format!(
                "Failed to insert file (id: {}, path: {}, url: {})",
                request.id.as_str(),
                request.path,
                request.public_url
            )
        })?;

        Ok(request.id)
    }

    pub async fn insert_file(&self, file: &File) -> Result<FileId> {
        let file_id = FileId::new(file.id.to_string());

        let request = InsertFileRequest {
            id: file_id.clone(),
            path: file.path.clone(),
            public_url: file.public_url.clone(),
            mime_type: file.mime_type.clone(),
            size_bytes: file.size_bytes,
            ai_content: file.ai_content,
            metadata: file.metadata.clone(),
            user_id: file.user_id.clone(),
            session_id: file.session_id.clone(),
            trace_id: file.trace_id.clone(),
        };

        self.insert(request).await
    }

    pub async fn find_by_id(&self, id: &FileId) -> Result<Option<File>> {
        let id_uuid = uuid::Uuid::parse_str(id.as_str()).context("Invalid UUID for file id")?;

        sqlx::query_as!(
            File,
            r#"
            SELECT id, path, public_url, mime_type, size_bytes, ai_content, metadata, user_id as "user_id: UserId", session_id as "session_id: SessionId", trace_id as "trace_id: TraceId", created_at, updated_at, deleted_at
            FROM files
            WHERE id = $1 AND deleted_at IS NULL
            "#,
            id_uuid
        )
        .fetch_optional(self.pool.as_ref())
        .await
        .context(format!("Failed to find file by id: {id}"))
    }

    pub async fn find_by_path(&self, path: &str) -> Result<Option<File>> {
        sqlx::query_as!(
            File,
            r#"
            SELECT id, path, public_url, mime_type, size_bytes, ai_content, metadata, user_id as "user_id: UserId", session_id as "session_id: SessionId", trace_id as "trace_id: TraceId", created_at, updated_at, deleted_at
            FROM files
            WHERE path = $1 AND deleted_at IS NULL
            "#,
            path
        )
        .fetch_optional(self.pool.as_ref())
        .await
        .context(format!("Failed to find file by path: {path}"))
    }

    pub async fn list_by_user(
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
            WHERE user_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            user_id_str,
            limit,
            offset
        )
        .fetch_all(self.pool.as_ref())
        .await
        .context(format!("Failed to list files for user: {user_id}"))
    }

    pub async fn list_all(&self, limit: i64, offset: i64) -> Result<Vec<File>> {
        sqlx::query_as!(
            File,
            r#"
            SELECT id, path, public_url, mime_type, size_bytes, ai_content, metadata, user_id as "user_id: UserId", session_id as "session_id: SessionId", trace_id as "trace_id: TraceId", created_at, updated_at, deleted_at
            FROM files
            WHERE deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(self.pool.as_ref())
        .await
        .context("Failed to list all files")
    }

    pub async fn soft_delete(&self, id: &FileId) -> Result<()> {
        let id_uuid = uuid::Uuid::parse_str(id.as_str()).context("Invalid UUID for file id")?;
        let now = Utc::now();

        sqlx::query!(
            r#"
            UPDATE files
            SET deleted_at = $1, updated_at = $1
            WHERE id = $2
            "#,
            now,
            id_uuid
        )
        .execute(self.pool.as_ref())
        .await
        .context(format!("Failed to soft delete file: {id}"))?;

        Ok(())
    }

    pub async fn update_metadata(&self, id: &FileId, metadata: &FileMetadata) -> Result<()> {
        let id_uuid = uuid::Uuid::parse_str(id.as_str()).context("Invalid UUID for file id")?;
        let metadata_json = serde_json::to_value(metadata)?;
        let now = Utc::now();

        sqlx::query!(
            r#"
            UPDATE files
            SET metadata = $1, updated_at = $2
            WHERE id = $3
            "#,
            metadata_json,
            now,
            id_uuid
        )
        .execute(self.pool.as_ref())
        .await
        .context(format!("Failed to update metadata for file: {id}"))?;

        Ok(())
    }
}
