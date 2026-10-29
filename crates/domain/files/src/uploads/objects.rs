//! The objects an upload has stored but no part row or blob holds yet.
//!
//! A part attempt is recorded here before its bytes are stored and
//! forgotten once its part row holds it or its object is deleted; an
//! object whose delete failed is recorded again. Whatever an interrupted
//! attempt or a failed delete left behind is therefore listed by
//! [`DocumentUploadRepository::list_orphaned_objects`], never lost. An
//! object a part row of an `open` or `completing` upload still holds is
//! never listed, and one an `open` upload's attempt may still be writing is
//! listed only once it is older than [`ATTEMPT_GRACE_HOURS`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentUploadId;
use systemprompt_traits::{RepositoryError, StoredFileId};

use super::repository::DocumentUploadRepository;

type RepoResult<T> = Result<T, RepositoryError>;

pub const ATTEMPT_GRACE_HOURS: i32 = 6;

/// An upload object no part row or blob holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrphanedObject {
    pub upload_id: DocumentUploadId,
    pub storage_id: StoredFileId,
}

struct OrphanRow {
    upload_id: DocumentUploadId,
    storage_id: String,
}

impl DocumentUploadRepository {
    pub async fn track_object(
        &self,
        upload: &DocumentUploadId,
        object: &StoredFileId,
    ) -> RepoResult<()> {
        sqlx::query!(
            "INSERT INTO document_upload_objects (storage_id, upload_id)
             VALUES ($1, $2::text::uuid)
             ON CONFLICT (storage_id) DO NOTHING",
            object.as_str(),
            upload.as_str(),
        )
        .execute(&*self.write)
        .await?;
        Ok(())
    }

    pub async fn untrack_object(&self, object: &StoredFileId) -> RepoResult<()> {
        sqlx::query!(
            "DELETE FROM document_upload_objects WHERE storage_id = $1",
            object.as_str(),
        )
        .execute(&*self.write)
        .await?;
        Ok(())
    }

    pub async fn list_orphaned_objects(&self, limit: i64) -> RepoResult<Vec<OrphanedObject>> {
        let rows = sqlx::query_as!(
            OrphanRow,
            r#"SELECT o.upload_id::text AS "upload_id!: DocumentUploadId", o.storage_id
               FROM document_upload_objects o
               WHERE NOT EXISTS (
                       SELECT 1 FROM document_upload_parts p
                       JOIN document_uploads u ON u.id = p.upload_id
                       WHERE p.storage_id = o.storage_id
                         AND u.status IN ('open', 'completing'))
                 AND (o.created_at < now() - make_interval(hours => $2)
                      OR NOT EXISTS (SELECT 1 FROM document_uploads u
                                     WHERE u.id = o.upload_id AND u.status = 'open'))
               ORDER BY o.created_at LIMIT $1"#,
            limit,
            ATTEMPT_GRACE_HOURS,
        )
        .fetch_all(&*self.write)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| OrphanedObject {
                upload_id: row.upload_id,
                storage_id: StoredFileId::new(row.storage_id),
            })
            .collect())
    }
}
