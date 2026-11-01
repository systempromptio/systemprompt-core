//! Recording a stored part attempt. Every attempt is stored under its own
//! object (`part-NNNNN-<attempt>`), so a failed or racing re-send deletes
//! only its own bytes, never a part already accepted. The row swap answers
//! the object it replaced, and the caller deletes that one only after the
//! row points at the new one. The swap first locks the upload's row in its
//! own statement, so swaps of one upload are serial and the swap that
//! follows reads (under READ COMMITTED) the part row as the last swap left
//! it: two first attempts of a part cannot both see no previous object and
//! orphan the loser's. A refund takes the same row lock and skips an attempt
//! whose object a committed swap already recorded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentUploadId;
use systemprompt_traits::{RepositoryError, StoredFileId};

use super::models::UploadPart;
use super::repository::DocumentUploadRepository;

type RepoResult<T> = Result<T, RepositoryError>;

/// What [`DocumentUploadRepository::swap_part`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartSwap {
    pub written: bool,
    pub previous: Option<String>,
}

struct SwapRow {
    written: i64,
    previous: Option<String>,
}

impl DocumentUploadRepository {
    pub async fn swap_part(
        &self,
        id: &DocumentUploadId,
        part: &UploadPart,
    ) -> RepoResult<PartSwap> {
        let number = i32::try_from(part.part_number).map_err(|source| {
            RepositoryError::decode("document_upload_parts.part_number", source)
        })?;
        let size = i64::try_from(part.size_bytes).map_err(|source| {
            RepositoryError::decode("document_upload_parts.size_bytes", source)
        })?;
        let mut tx = self.write.begin().await?;
        sqlx::query_scalar!(
            r#"SELECT 1 AS "locked!" FROM document_uploads WHERE id = $1::text::uuid FOR UPDATE"#,
            id.as_str(),
        )
        .fetch_optional(&mut *tx)
        .await?;
        let row = sqlx::query_as!(
            SwapRow,
            r#"WITH previous AS (
                   SELECT storage_id FROM document_upload_parts
                   WHERE upload_id = $1::text::uuid AND part_number = $2
                   FOR UPDATE
               ),
               written AS (
                   INSERT INTO document_upload_parts
                       (upload_id, part_number, size_bytes, sha256, storage_id)
                   SELECT $1::text::uuid, $2, $3, $4, $5
                   WHERE EXISTS (SELECT 1 FROM document_uploads
                                 WHERE id = $1::text::uuid AND status = 'open')
                   ON CONFLICT (upload_id, part_number) DO UPDATE
                       SET size_bytes = EXCLUDED.size_bytes, sha256 = EXCLUDED.sha256,
                           storage_id = EXCLUDED.storage_id, created_at = CURRENT_TIMESTAMP
                   RETURNING 1
               )
               SELECT (SELECT COUNT(*) FROM written)::bigint AS "written!",
                      (SELECT storage_id FROM previous) AS previous"#,
            id.as_str(),
            number,
            size,
            part.sha256.as_str(),
            part.storage_id.as_str(),
        )
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(PartSwap {
            written: row.written == 1,
            previous: row
                .previous
                .filter(|previous| *previous != part.storage_id.as_str()),
        })
    }

    pub async fn refund_part_bytes(
        &self,
        id: &DocumentUploadId,
        attempt: Option<&StoredFileId>,
        bytes: u64,
    ) -> RepoResult<()> {
        let bytes = i64::try_from(bytes)
            .map_err(|source| RepositoryError::decode("document_uploads.received_bytes", source))?;
        let mut tx = self.write.begin().await?;
        sqlx::query_scalar!(
            r#"SELECT 1 AS "locked!" FROM document_uploads WHERE id = $1::text::uuid FOR UPDATE"#,
            id.as_str(),
        )
        .fetch_optional(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE document_uploads SET received_bytes = GREATEST(received_bytes - $2, 0)
             WHERE id = $1::text::uuid
               AND ($3::text IS NULL OR NOT EXISTS (
                   SELECT 1 FROM document_upload_parts
                   WHERE upload_id = $1::text::uuid AND storage_id = $3))",
            id.as_str(),
            bytes,
            attempt.map(StoredFileId::as_str),
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
