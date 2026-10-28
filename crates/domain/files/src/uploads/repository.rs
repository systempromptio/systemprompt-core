//! [`DocumentUploadRepository`]: upload sessions and their stored parts.
//!
//! Every state change names the state it leaves, so two calls racing on one
//! upload have one winner and the loser sees zero rows affected. A
//! completion first claims the upload (`open` -> `completing`); abort only
//! closes an `open` upload, so it can never pull bytes out from under an
//! assembly. A claim older than an hour is a crashed completion, and the
//! expiry job closes it.
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
    pub(super) read: Arc<PgPool>,
    pub(super) write: Arc<PgPool>,
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
    checked_kind: Option<String>,
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
            checked_kind: row.checked_kind,
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

/// What [`DocumentUploadRepository::insert_admitted`] stores besides the
/// request, and the limits it admits the session under.
#[derive(Debug, Clone, Copy)]
pub struct SessionTerms<'a> {
    pub owner: &'a UserId,
    pub part_size: i64,
    pub ttl_hours: i32,
    pub trace: &'a TraceId,
    pub max_open: i64,
    pub daily_quota: i64,
    pub one_shot: bool,
    pub quota_exempt: bool,
}

/// Whether [`DocumentUploadRepository::insert_admitted`] opened the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    Admitted(Box<UploadSession>),
    TooManyOpen { open: u64 },
    QuotaExceeded { used: u64 },
}

impl DocumentUploadRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            read: db.pool(),
            write: db.write_pool(),
        }
    }

    pub async fn insert_admitted(
        &self,
        request: &NewUpload,
        terms: SessionTerms<'_>,
    ) -> RepoResult<Admission> {
        let size = i64::try_from(request.size)
            .map_err(|source| RepositoryError::decode("document_uploads.declared_size", source))?;
        let mut tx = self.write.begin().await?;
        sqlx::query_scalar!(
            r#"SELECT 1 AS "locked!" FROM (SELECT pg_advisory_xact_lock(hashtext($1))) AS lock"#,
            terms.owner.as_str(),
        )
        .fetch_one(&mut *tx)
        .await?;
        let open = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint AS "count!" FROM document_uploads
               WHERE owner_user_id = $1 AND status IN ('open', 'completing')
                 AND NOT one_shot AND expires_at > now()"#,
            terms.owner.as_str(),
        )
        .fetch_one(&mut *tx)
        .await?;
        if !terms.one_shot && open >= terms.max_open {
            return Ok(Admission::TooManyOpen {
                open: open.unsigned_abs(),
            });
        }
        let used = sqlx::query_scalar!(
            r#"SELECT COALESCE(SUM(CASE WHEN status = 'aborted' THEN received_bytes
                                        ELSE declared_size END), 0)::bigint AS "total!"
               FROM document_uploads
               WHERE owner_user_id = $1 AND NOT quota_exempt
                 AND created_at > now() - interval '1 day'"#,
            terms.owner.as_str(),
        )
        .fetch_one(&mut *tx)
        .await?;
        if !terms.quota_exempt && used.saturating_add(size) > terms.daily_quota {
            return Ok(Admission::QuotaExceeded {
                used: used.unsigned_abs(),
            });
        }
        let session: UploadSession = sqlx::query_as!(
            SessionRow,
            r#"INSERT INTO document_uploads
                   (owner_user_id, file_name, declared_mime, declared_size, declared_sha256,
                    part_size, trace_id, expires_at, one_shot, quota_exempt)
               VALUES ($1, $2, $3, $4, $5, $6, $7, now() + make_interval(hours => $8::int), $9,
                       $10)
               RETURNING id::text AS "id!: DocumentUploadId",
                         owner_user_id AS "owner_user_id: UserId", file_name, declared_mime,
                         declared_size, declared_sha256 AS "declared_sha256: ContentHash",
                         part_size, status, blob_sha256 AS "blob_sha256: ContentHash",
                         detected_mime, checked_kind, refusal, trace_id AS "trace_id: TraceId", created_at,
                         expires_at, finished_at"#,
            terms.owner.as_str(),
            request.name,
            request.mime,
            size,
            request.sha256.as_str(),
            terms.part_size,
            terms.trace.as_str(),
            terms.ttl_hours,
            terms.one_shot,
            terms.quota_exempt,
        )
        .fetch_one(&mut *tx)
        .await?
        .try_into()?;
        tx.commit().await?;
        Ok(Admission::Admitted(Box::new(session)))
    }

    pub async fn find(&self, id: &DocumentUploadId) -> RepoResult<Option<UploadSession>> {
        sqlx::query_as!(
            SessionRow,
            r#"SELECT id::text AS "id!: DocumentUploadId",
                      owner_user_id AS "owner_user_id: UserId", file_name, declared_mime,
                      declared_size, declared_sha256 AS "declared_sha256: ContentHash",
                      part_size, status, blob_sha256 AS "blob_sha256: ContentHash",
                      detected_mime, checked_kind, refusal, trace_id AS "trace_id: TraceId", created_at,
                      expires_at, finished_at
               FROM document_uploads WHERE id = $1::text::uuid"#,
            id.as_str(),
        )
        .fetch_optional(&*self.write)
        .await?
        .map(UploadSession::try_from)
        .transpose()
    }

    pub async fn reserve_part_bytes(&self, id: &DocumentUploadId, bytes: u64) -> RepoResult<bool> {
        let bytes = i64::try_from(bytes)
            .map_err(|source| RepositoryError::decode("document_uploads.received_bytes", source))?;
        let result = sqlx::query!(
            "UPDATE document_uploads SET received_bytes = received_bytes + $2
             WHERE id = $1::text::uuid AND status = 'open'
               AND received_bytes + $2 <= declared_size * 2",
            id.as_str(),
            bytes,
        )
        .execute(&*self.write)
        .await?;
        Ok(result.rows_affected() == 1)
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
}
