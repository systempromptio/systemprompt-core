use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};

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
    pub context_id: Option<ContextId>,
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
            context_id: None,
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

    pub fn with_context_id(mut self, context_id: ContextId) -> Self {
        self.context_id = Some(context_id);
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
        let context_id_str = request.context_id.as_ref().map(ContextId::as_str);

        sqlx::query_as!(
            File,
            r#"
            INSERT INTO files (id, path, public_url, mime_type, size_bytes, ai_content, metadata, user_id, session_id, trace_id, context_id, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12)
            ON CONFLICT (path) DO UPDATE SET
                public_url = EXCLUDED.public_url,
                mime_type = EXCLUDED.mime_type,
                size_bytes = EXCLUDED.size_bytes,
                ai_content = EXCLUDED.ai_content,
                metadata = EXCLUDED.metadata,
                updated_at = EXCLUDED.updated_at
            RETURNING id, path, public_url, mime_type, size_bytes, ai_content, metadata, user_id as "user_id: UserId", session_id as "session_id: SessionId", trace_id as "trace_id: TraceId", context_id as "context_id: ContextId", created_at, updated_at, deleted_at
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
            context_id_str,
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

        let mut request = InsertFileRequest::new(
            file_id.clone(),
            file.path.clone(),
            file.public_url.clone(),
            file.mime_type.clone(),
        )
        .with_ai_content(file.ai_content)
        .with_metadata(file.metadata.clone());

        if let Some(size) = file.size_bytes {
            request = request.with_size(size);
        }

        if let Some(ref user_id) = file.user_id {
            request = request.with_user_id(user_id.clone());
        }

        if let Some(ref session_id) = file.session_id {
            request = request.with_session_id(session_id.clone());
        }

        if let Some(ref trace_id) = file.trace_id {
            request = request.with_trace_id(trace_id.clone());
        }

        if let Some(ref context_id) = file.context_id {
            request = request.with_context_id(context_id.clone());
        }

        self.insert(request).await
    }

    pub async fn find_by_id(&self, id: &FileId) -> Result<Option<File>> {
        let id_uuid = uuid::Uuid::parse_str(id.as_str()).context("Invalid UUID for file id")?;

        sqlx::query_as!(
            File,
            r#"
            SELECT id, path, public_url, mime_type, size_bytes, ai_content, metadata, user_id as "user_id: UserId", session_id as "session_id: SessionId", trace_id as "trace_id: TraceId", context_id as "context_id: ContextId", created_at, updated_at, deleted_at
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
            SELECT id, path, public_url, mime_type, size_bytes, ai_content, metadata, user_id as "user_id: UserId", session_id as "session_id: SessionId", trace_id as "trace_id: TraceId", context_id as "context_id: ContextId", created_at, updated_at, deleted_at
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
            SELECT id, path, public_url, mime_type, size_bytes, ai_content, metadata, user_id as "user_id: UserId", session_id as "session_id: SessionId", trace_id as "trace_id: TraceId", context_id as "context_id: ContextId", created_at, updated_at, deleted_at
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
            SELECT id, path, public_url, mime_type, size_bytes, ai_content, metadata, user_id as "user_id: UserId", session_id as "session_id: SessionId", trace_id as "trace_id: TraceId", context_id as "context_id: ContextId", created_at, updated_at, deleted_at
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

    pub async fn delete(&self, id: &FileId) -> Result<()> {
        let id_uuid = uuid::Uuid::parse_str(id.as_str()).context("Invalid UUID for file id")?;

        sqlx::query!(
            r#"
            DELETE FROM files
            WHERE id = $1
            "#,
            id_uuid
        )
        .execute(self.pool.as_ref())
        .await
        .context(format!("Failed to delete file: {id}"))?;

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

    pub async fn get_stats(&self) -> Result<FileStats> {
        let row = sqlx::query!(
            r#"
            SELECT
                COUNT(*) as "total_files!",
                COALESCE(SUM(size_bytes), 0)::bigint as "total_size_bytes!",
                COUNT(*) FILTER (WHERE ai_content = true) as "ai_images_count!",
                COUNT(*) FILTER (WHERE mime_type LIKE 'image/%') as "image_count!",
                COALESCE(SUM(size_bytes) FILTER (WHERE mime_type LIKE 'image/%'), 0)::bigint as "image_size!",
                COUNT(*) FILTER (WHERE mime_type LIKE 'application/pdf' OR mime_type LIKE 'application/msword%' OR mime_type LIKE 'application/vnd.openxmlformats%' OR mime_type LIKE 'text/%') as "document_count!",
                COALESCE(SUM(size_bytes) FILTER (WHERE mime_type LIKE 'application/pdf' OR mime_type LIKE 'application/msword%' OR mime_type LIKE 'application/vnd.openxmlformats%' OR mime_type LIKE 'text/%'), 0)::bigint as "document_size!",
                COUNT(*) FILTER (WHERE mime_type LIKE 'audio/%') as "audio_count!",
                COALESCE(SUM(size_bytes) FILTER (WHERE mime_type LIKE 'audio/%'), 0)::bigint as "audio_size!",
                COUNT(*) FILTER (WHERE mime_type LIKE 'video/%') as "video_count!",
                COALESCE(SUM(size_bytes) FILTER (WHERE mime_type LIKE 'video/%'), 0)::bigint as "video_size!"
            FROM files
            WHERE deleted_at IS NULL
            "#
        )
        .fetch_one(self.pool.as_ref())
        .await
        .context("Failed to get file stats")?;

        let image_count = row.image_count;
        let document_count = row.document_count;
        let audio_count = row.audio_count;
        let video_count = row.video_count;
        let other_count =
            (row.total_files - image_count - document_count - audio_count - video_count).max(0);

        let image_size = row.image_size;
        let document_size = row.document_size;
        let audio_size = row.audio_size;
        let video_size = row.video_size;
        let other_size =
            (row.total_size_bytes - image_size - document_size - audio_size - video_size).max(0);

        Ok(FileStats {
            total_files: row.total_files,
            total_size_bytes: row.total_size_bytes,
            ai_images_count: row.ai_images_count,
            image_count,
            image_size_bytes: image_size,
            document_count,
            document_size_bytes: document_size,
            audio_count,
            audio_size_bytes: audio_size,
            video_count,
            video_size_bytes: video_size,
            other_count,
            other_size_bytes: other_size,
        })
    }

    pub async fn search_by_path(&self, query: &str, limit: i64) -> Result<Vec<File>> {
        let pattern = format!("%{query}%");
        sqlx::query_as!(
            File,
            r#"
            SELECT id, path, public_url, mime_type, size_bytes, ai_content, metadata,
                   user_id as "user_id: UserId", session_id as "session_id: SessionId",
                   trace_id as "trace_id: TraceId", context_id as "context_id: ContextId",
                   created_at, updated_at, deleted_at
            FROM files
            WHERE path ILIKE $1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            pattern,
            limit
        )
        .fetch_all(self.pool.as_ref())
        .await
        .context(format!("Failed to search files by path: {query}"))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FileStats {
    pub total_files: i64,
    pub total_size_bytes: i64,
    pub ai_images_count: i64,
    pub image_count: i64,
    pub image_size_bytes: i64,
    pub document_count: i64,
    pub document_size_bytes: i64,
    pub audio_count: i64,
    pub audio_size_bytes: i64,
    pub video_count: i64,
    pub video_size_bytes: i64,
    pub other_count: i64,
    pub other_size_bytes: i64,
}
