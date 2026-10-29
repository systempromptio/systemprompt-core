//! The state changes of an upload session: the completion claim and its
//! heartbeat, the terminal marks, expiry, the recorded content check, and
//! withdrawing a blob a lost completion created.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ContentHash, DocumentUploadId, UserId};
use systemprompt_traits::RepositoryError;

use super::models::{BlobOrigin, UploadStatus};
use super::repository::DocumentUploadRepository;

type RepoResult<T> = Result<T, RepositoryError>;

impl DocumentUploadRepository {
    pub async fn claim(&self, id: &DocumentUploadId) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_uploads SET status = 'completing', claimed_at = CURRENT_TIMESTAMP
             WHERE id = $1::text::uuid AND status = 'open' AND expires_at > now()",
            id.as_str(),
        )
        .execute(&*self.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn touch_claim(&self, id: &DocumentUploadId) -> RepoResult<()> {
        sqlx::query!(
            "UPDATE document_uploads SET claimed_at = CURRENT_TIMESTAMP
             WHERE id = $1::text::uuid AND status = 'completing'",
            id.as_str(),
        )
        .execute(&*self.write)
        .await?;
        Ok(())
    }

    pub async fn set_checked_kind(
        &self,
        owner: &UserId,
        blob: &ContentHash,
        checked_kind: &str,
    ) -> RepoResult<()> {
        sqlx::query!(
            "UPDATE document_uploads SET checked_kind = $3
             WHERE owner_user_id = $1 AND blob_sha256 = $2 AND status = 'completed'
               AND checked_kind IS NULL",
            owner.as_str(),
            blob.as_str(),
            checked_kind,
        )
        .execute(&*self.write)
        .await?;
        Ok(())
    }

    pub async fn release(&self, id: &DocumentUploadId) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_uploads SET status = 'open', claimed_at = NULL
             WHERE id = $1::text::uuid AND status = 'completing'",
            id.as_str(),
        )
        .execute(&*self.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn mark_completed(
        &self,
        id: &DocumentUploadId,
        blob: &ContentHash,
        detected_mime: &str,
        checked_kind: &str,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_uploads
             SET status = 'completed', blob_sha256 = $2, detected_mime = $3, checked_kind = $4,
                 finished_at = CURRENT_TIMESTAMP
             WHERE id = $1::text::uuid AND status = 'completing'",
            id.as_str(),
            blob.as_str(),
            detected_mime,
            checked_kind,
        )
        .execute(&*self.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn mark_closed(
        &self,
        id: &DocumentUploadId,
        from: UploadStatus,
        status: UploadStatus,
        refusal: Option<&str>,
    ) -> RepoResult<bool> {
        let result = sqlx::query!(
            "UPDATE document_uploads
             SET status = $2, refusal = $3, finished_at = CURRENT_TIMESTAMP
             WHERE id = $1::text::uuid AND status = $4",
            id.as_str(),
            status.as_str(),
            refusal,
            from.as_str(),
        )
        .execute(&*self.write)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn list_expired(&self, limit: i64) -> RepoResult<Vec<DocumentUploadId>> {
        Ok(sqlx::query_scalar!(
            r#"SELECT u.id::text AS "id!: DocumentUploadId" FROM (
                   (SELECT id, expires_at FROM document_uploads
                    WHERE status = 'open' AND expires_at < now()
                    ORDER BY expires_at LIMIT $1)
                   UNION ALL
                   (SELECT id, expires_at FROM document_uploads
                    WHERE status = 'completing' AND claimed_at < now() - interval '1 hour'
                    ORDER BY claimed_at LIMIT $1)
               ) u
               ORDER BY u.expires_at LIMIT $1"#,
            limit,
        )
        .fetch_all(&*self.read)
        .await?)
    }

    pub async fn withdraw_blob(&self, blob: &ContentHash) -> RepoResult<Option<String>> {
        Ok(sqlx::query_scalar!(
            r#"DELETE FROM document_blobs b WHERE b.content_sha256 = $1
                 AND NOT EXISTS (SELECT 1 FROM document_revisions r
                                 WHERE r.content_sha256 = b.content_sha256)
                 AND NOT EXISTS (SELECT 1 FROM document_edit_sessions s
                                 WHERE s.snapshot_sha256 = b.content_sha256)
                 AND NOT EXISTS (SELECT 1 FROM document_uploads u
                                 WHERE u.blob_sha256 = b.content_sha256
                                   AND u.status = 'completed')
                 AND NOT EXISTS (SELECT 1 FROM document_uploads c
                                 WHERE c.declared_sha256 = b.content_sha256
                                   AND c.status = 'completing')
               RETURNING b.storage_id"#,
            blob.as_str(),
        )
        .fetch_optional(&*self.write)
        .await?)
    }

    pub async fn find_blob_origin(
        &self,
        owner: &UserId,
        blob: &ContentHash,
    ) -> RepoResult<Option<BlobOrigin>> {
        Ok(sqlx::query_as!(
            BlobOrigin,
            r#"SELECT COALESCE(detected_mime, declared_mime) AS "mime!", checked_kind
               FROM document_uploads
               WHERE owner_user_id = $1 AND blob_sha256 = $2 AND status = 'completed'
               ORDER BY finished_at DESC LIMIT 1"#,
            owner.as_str(),
            blob.as_str(),
        )
        .fetch_optional(&*self.write)
        .await?)
    }
}
