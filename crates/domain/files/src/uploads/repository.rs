//! [`DocumentUploadRepository`]: upload sessions and their stored parts.
//!
//! A session leaves `open` exactly once: every terminal update is guarded by
//! `status = 'open'`, so a completion racing an abort or the expiry job has
//! one winner and the loser sees zero rows affected.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentHash, DocumentUploadId, TraceId, UserId};
use systemprompt_traits::RepositoryError;

use super::models::{NewUpload, UploadPart, UploadSession, UploadStatus};

type RepoResult<T> = Result<T, RepositoryError>;

#[derive(Debug, Clone)]
pub struct DocumentUploadRepository {
    read: Arc<PgPool>,
    write: Arc<PgPool>,
}

struct SessionRow {
    id: DocumentUploadId,
    owner_user_id: UserId,
    file_name: String,
    declared_mime: String,
    declared_size: i64,
    declared_sha256: ContentHash,
    part_size: i64,
    status: String,
    blob_sha256: Option<ContentHash>,
    detected_mime: Option<String>,
    refusal: Option<String>,
    trace_id: TraceId,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

impl TryFrom<SessionRow> for UploadSession {
    type Error = RepositoryError;

    fn try_from(row: SessionRow) -> RepoResult<Self> {
        Ok(Self {
            status: row
                .status
                .parse::<UploadStatus>()
                .map_err(|source| RepositoryError::decode("document_uploads.status", source))?,
            id: row.id,
            owner_user_id: row.owner_user_id,
            file_name: row.file_name,
            declared_mime: row.declared_mime,
            declared_size: row.declared_size.unsigned_abs(),
            declared_sha256: row.declared_sha256,
            part_size: row.part_size.unsigned_abs(),
            blob_sha256: row.blob_sha256,
            detected_mime: row.detected_mime,
            refusal: row.refusal,
            trace_id: row.trace_id,
            created_at: row.created_at,
            expires_at: row.expires_at,
            finished_at: row.finished_at,
        })
    }
}

struct PartRow {
    part_number: i32,
    size_bytes: i64,
    sha256: ContentHash,
    storage_id: String,
}

impl From<PartRow> for UploadPart {
    fn from(row: PartRow) -> Self {
        Self {
            part_number: row.part_number.unsigned_abs(),
            size_bytes: row.size_bytes.unsigned_abs(),
            sha256: row.sha256,
            storage_id: row.storage_id,
        }
    }
}

/// What [`DocumentUploadRepository::insert`] stores besides the request.
#[derive(Debug, Clone, Copy)]
pub struct SessionTerms<'a> {
    pub owner: &'a UserId,
    pub part_size: i64,
    pub ttl_hours: i32,
    pub trace: &'a TraceId,
}

impl DocumentUploadRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            read: db.pool(),
            write: db.write_pool(),
        }
    }

    pub async fn insert(
        &self,
        request: &NewUpload,
        terms: SessionTerms<'_>,
    ) -> RepoResult<UploadSession> {
        let size = i64::try_from(request.size)
            .map_err(|source| RepositoryError::decode("document_uploads.declared_size", source))?;
        sqlx::query_as!(
            SessionRow,
            r#"INSERT INTO document_uploads
                   (owner_user_id, file_name, declared_mime, declared_size, declared_sha256,
                    part_size, trace_id, expires_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, now() + make_interval(hours => $8::int))
               RETURNING id::text AS "id!: DocumentUploadId",
                         owner_user_id AS "owner_user_id: UserId", file_name, declared_mime,
                         declared_size, declared_sha256 AS "declared_sha256: ContentHash",
                         part_size, status, blob_sha256 AS "blob_sha256: ContentHash",
                         detected_mime, refusal, trace_id AS "trace_id: TraceId", created_at,
                         expires_at, finished_at"#,
            terms.owner.as_str(),
            request.name,
            request.mime,
            size,
            request.sha256.as_str(),
            terms.part_size,
            terms.trace.as_str(),
            terms.ttl_hours,
        )
        .fetch_one(&*self.write)
        .await?
        .try_into()
    }

    pub async fn find(&self, id: &DocumentUploadId) -> RepoResult<Option<UploadSession>> {
        sqlx::query_as!(
            SessionRow,
            r#"SELECT id::text AS "id!: DocumentUploadId",
                      owner_user_id AS "owner_user_id: UserId", file_name, declared_mime,
                      declared_size, declared_sha256 AS "declared_sha256: ContentHash",
                      part_size, status, blob_sha256 AS "blob_sha256: ContentHash",
                      detected_mime, refusal, trace_id AS "trace_id: TraceId", created_at,
                      expires_at, finished_at
               FROM document_uploads WHERE id = $1::text::uuid"#,
            id.as_str(),
        )
        .fetch_optional(&*self.write)
        .await?
        .map(UploadSession::try_from)
        .transpose()
    }

    pub async fn count_open(&self, owner: &UserId) -> RepoResult<i64> {
        Ok(sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint AS "count!" FROM document_uploads
               WHERE owner_user_id = $1 AND status = 'open' AND expires_at > now()"#,
            owner.as_str(),
        )
        .fetch_one(&*self.write)
        .await?)
    }

    pub async fn bytes_last_day(&self, owner: &UserId) -> RepoResult<i64> {
        Ok(sqlx::query_scalar!(
            r#"SELECT COALESCE(SUM(declared_size), 0)::bigint AS "total!" FROM document_uploads
               WHERE owner_user_id = $1 AND status IN ('open', 'completed')
                 AND created_at > now() - interval '1 day'"#,
            owner.as_str(),
        )
        .fetch_one(&*self.write)
        .await?)
    }

    pub async fn upsert_part(&self, id: &DocumentUploadId, part: &UploadPart) -> RepoResult<()> {
        let number = i32::try_from(part.part_number).map_err(|source| {
            RepositoryError::decode("document_upload_parts.part_number", source)
        })?;
        let size = i64::try_from(part.size_bytes).map_err(|source| {
            RepositoryError::decode("document_upload_parts.size_bytes", source)
        })?;
        sqlx::query!(
            "INSERT INTO document_upload_parts (upload_id, part_number, size_bytes, sha256, storage_id)
             VALUES ($1::text::uuid, $2, $3, $4, $5)
             ON CONFLICT (upload_id, part_number) DO UPDATE
                 SET size_bytes = EXCLUDED.size_bytes, sha256 = EXCLUDED.sha256,
                     storage_id = EXCLUDED.storage_id, created_at = CURRENT_TIMESTAMP",
            id.as_str(),
            number,
            size,
            part.sha256.as_str(),
            part.storage_id,
        )
        .execute(&*self.write)
        .await?;
        Ok(())
    }

    pub async fn list_parts(&self, id: &DocumentUploadId) -> RepoResult<Vec<UploadPart>> {
        Ok(sqlx::query_as!(
            PartRow,
            r#"SELECT part_number, size_bytes, sha256 AS "sha256: ContentHash", storage_id
               FROM document_upload_parts WHERE upload_id = $1::text::uuid
               ORDER BY part_number"#,
            id.as_str(),
        )
        .fetch_all(&*self.write)
        .await?
        .into_iter()
        .map(UploadPart::from)
        .collect())
    }

    pub async fn mark_completed(
        &self,
        id: &DocumentUploadId,
        blob: &ContentHash,
        detected_mime: &str,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_uploads
             SET status = 'completed', blob_sha256 = $2, detected_mime = $3,
                 finished_at = CURRENT_TIMESTAMP
             WHERE id = $1::text::uuid AND status = 'open'",
            id.as_str(),
            blob.as_str(),
            detected_mime,
        )
        .execute(&*self.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn mark_closed(
        &self,
        id: &DocumentUploadId,
        status: UploadStatus,
        refusal: Option<&str>,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_uploads
             SET status = $2, refusal = $3, finished_at = CURRENT_TIMESTAMP
             WHERE id = $1::text::uuid AND status = 'open'",
            id.as_str(),
            status.as_str(),
            refusal,
        )
        .execute(&*self.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn list_expired(&self, limit: i64) -> RepoResult<Vec<DocumentUploadId>> {
        Ok(sqlx::query_scalar!(
            r#"SELECT id::text AS "id!: DocumentUploadId" FROM document_uploads
               WHERE status = 'open' AND expires_at < now()
               ORDER BY expires_at LIMIT $1"#,
            limit,
        )
        .fetch_all(&*self.read)
        .await?)
    }

    pub async fn owns_blob(&self, owner: &UserId, blob: &ContentHash) -> RepoResult<bool> {
        Ok(sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM document_uploads
                              WHERE owner_user_id = $1 AND blob_sha256 = $2
                                AND status = 'completed') AS "owned!""#,
            owner.as_str(),
            blob.as_str(),
        )
        .fetch_one(&*self.write)
        .await?)
    }
}
