//! [`FileRepository`] queries for file/content associations.
//!
//! Linking and unlinking files to content, listing the files for a piece of
//! content (and vice versa), and managing the single featured-image role.

use chrono::Utc;
use systemprompt_identifiers::{ContentId, ContextId, FileId, SessionId, TraceId, UserId};

use super::file::FileRepository;
use crate::error::{FilesError, FilesResult};
use crate::models::{ContentFile, File, FileMetadata, FileRole};

impl FileRepository {
    pub async fn link_to_content(
        &self,
        content_id: &ContentId,
        file_id: &FileId,
        role: FileRole,
        display_order: i32,
    ) -> FilesResult<ContentFile> {
        let file_id_uuid = uuid::Uuid::parse_str(file_id.as_str())
            .map_err(|e| FilesError::Validation(format!("Invalid UUID for file id: {e}")))?;
        let now = Utc::now();
        let content_id_str = content_id.as_str();

        let result = sqlx::query_as!(
            ContentFile,
            r#"
            INSERT INTO content_files (content_id, file_id, role, display_order, created_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (content_id, file_id, role) DO UPDATE
            SET display_order = $4
            RETURNING id, content_id as "content_id: ContentId", file_id, role as "role: FileRole", display_order, created_at
            "#,
            content_id_str,
            file_id_uuid,
            role.as_str(),
            display_order,
            now
        )
        .fetch_one(self.pool.as_ref())
        .await?;

        Ok(result)
    }

    pub async fn unlink_from_content(
        &self,
        content_id: &ContentId,
        file_id: &FileId,
    ) -> FilesResult<()> {
        let file_id_uuid = uuid::Uuid::parse_str(file_id.as_str())
            .map_err(|e| FilesError::Validation(format!("Invalid UUID for file id: {e}")))?;
        let content_id_str = content_id.as_str();

        sqlx::query!(
            r#"
            DELETE FROM content_files
            WHERE content_id = $1 AND file_id = $2
            "#,
            content_id_str,
            file_id_uuid
        )
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    pub async fn list_files_by_content(
        &self,
        content_id: &ContentId,
    ) -> FilesResult<Vec<(File, ContentFile)>> {
        let content_id_str = content_id.as_str();
        let rows = sqlx::query!(
            r#"
            SELECT
                f.id, f.path, f.public_url, f.mime_type, f.size_bytes, f.ai_content,
                f.metadata as "metadata: sqlx::types::Json<FileMetadata>", f.user_id, f.session_id, f.trace_id, f.context_id, f.created_at, f.updated_at, f.deleted_at,
                cf.id as cf_id, cf.content_id, cf.file_id as cf_file_id, cf.role as "role: FileRole", cf.display_order, cf.created_at as cf_created_at
            FROM files f
            INNER JOIN content_files cf ON cf.file_id = f.id
            WHERE cf.content_id = $1 AND f.deleted_at IS NULL
            ORDER BY cf.display_order ASC, cf.created_at ASC
            "#,
            content_id_str
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let file = File {
                    id: row.id,
                    path: row.path,
                    public_url: row.public_url,
                    mime_type: row.mime_type,
                    size_bytes: row.size_bytes,
                    ai_content: row.ai_content,
                    metadata: row.metadata,
                    user_id: row.user_id.map(UserId::new),
                    session_id: row.session_id.map(SessionId::new),
                    trace_id: row.trace_id.map(TraceId::new),
                    context_id: row.context_id.map(ContextId::new),
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    deleted_at: row.deleted_at,
                };

                let content_file = ContentFile {
                    id: row.cf_id,
                    content_id: ContentId::new(row.content_id),
                    file_id: row.cf_file_id,
                    role: row.role,
                    display_order: row.display_order,
                    created_at: row.cf_created_at,
                };

                (file, content_file)
            })
            .collect())
    }

    pub async fn find_featured_image(&self, content_id: &ContentId) -> FilesResult<Option<File>> {
        let content_id_str = content_id.as_str();
        let featured_role = FileRole::Featured.as_str();
        let result = sqlx::query_as!(
            File,
            r#"
            SELECT f.id, f.path, f.public_url, f.mime_type, f.size_bytes, f.ai_content,
                   f.metadata as "metadata: sqlx::types::Json<FileMetadata>", f.user_id as "user_id: UserId", f.session_id as "session_id: SessionId", f.trace_id as "trace_id: TraceId", f.context_id as "context_id: ContextId", f.created_at, f.updated_at, f.deleted_at
            FROM files f
            INNER JOIN content_files cf ON cf.file_id = f.id
            WHERE cf.content_id = $1
              AND cf.role = $2
              AND f.deleted_at IS NULL
            LIMIT 1
            "#,
            content_id_str,
            featured_role
        )
        .fetch_optional(self.pool.as_ref())
        .await?;

        Ok(result)
    }

    pub async fn set_featured(&self, file_id: &FileId, content_id: &ContentId) -> FilesResult<()> {
        let file_id_uuid = uuid::Uuid::parse_str(file_id.as_str())
            .map_err(|e| FilesError::Validation(format!("Invalid UUID for file id: {e}")))?;
        let content_id_str = content_id.as_str();
        let featured_role = FileRole::Featured.as_str();
        let attachment_role = FileRole::Attachment.as_str();

        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            r#"
            UPDATE content_files
            SET role = $1
            WHERE content_id = $2 AND role = $3
            "#,
            attachment_role,
            content_id_str,
            featured_role
        )
        .execute(&mut *tx)
        .await?;

        let result = sqlx::query!(
            r#"
            UPDATE content_files
            SET role = $1
            WHERE file_id = $2 AND content_id = $3
            "#,
            featured_role,
            file_id_uuid,
            content_id_str
        )
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(FilesError::NotFound(format!(
                "File {file_id} is not linked to content {content_id}"
            )));
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn list_content_by_file(&self, file_id: &FileId) -> FilesResult<Vec<ContentFile>> {
        let file_id_uuid = uuid::Uuid::parse_str(file_id.as_str())
            .map_err(|e| FilesError::Validation(format!("Invalid UUID for file id: {e}")))?;

        let result = sqlx::query_as!(
            ContentFile,
            r#"
            SELECT id, content_id as "content_id: ContentId", file_id, role as "role: FileRole", display_order, created_at
            FROM content_files
            WHERE file_id = $1
            ORDER BY created_at ASC
            "#,
            file_id_uuid
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(result)
    }
}
