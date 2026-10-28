//! One part attempt: stored under its own object, then swapped into the
//! part's row, and why a part was refused when it was not taken.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentUploadId;
use systemprompt_traits::{FileByteStream, StoredFileId};

use super::error::{UploadError, UploadResult};
use super::models::{UploadPart, UploadStatus};
use super::service::UploadService;
use super::stream::Tally;

impl UploadService {
    /// Stores one part attempt under its own object and points the part's
    /// row at it; a failed attempt deletes only its own object, and the part
    /// it replaces is deleted only once the row no longer names it.
    pub(super) async fn store_part(
        &self,
        id: &DocumentUploadId,
        part: u32,
        expected: u64,
        attempt: &StoredFileId,
        body: FileByteStream,
    ) -> UploadResult<UploadPart> {
        let tally = Tally::new(expected);
        let stored = self
            .storage
            .store_stream(std::path::Path::new(attempt.as_str()), tally.wrap(body))
            .await;
        let actual = tally.len();
        if tally.overflowed() || (stored.is_ok() && actual != expected) {
            self.discard(attempt).await;
            return Err(UploadError::PartSize {
                part,
                expected,
                actual,
            });
        }
        if let Err(error) = stored {
            self.discard(attempt).await;
            return Err(error.into());
        }
        let sha256 = tally
            .hash()
            .ok_or_else(|| UploadError::Invalid("part hash could not be formed".to_owned()))?;
        let stored_part = UploadPart {
            part_number: part,
            size_bytes: actual,
            sha256,
            storage_id: attempt.as_str().to_owned(),
        };
        let swap = self.uploads.swap_part(id, &stored_part).await?;
        if !swap.written {
            self.discard(attempt).await;
            return Err(self.part_refusal(id).await?);
        }
        if let Some(previous) = swap.previous {
            self.discard(&StoredFileId::new(previous)).await;
        }
        Ok(stored_part)
    }

    /// Why a part was not taken: the session left `open`, or it spent the
    /// part budget re-sending parts.
    pub(super) async fn part_refusal(&self, id: &DocumentUploadId) -> UploadResult<UploadError> {
        let session = self.uploads.find(id).await?;
        Ok(match session {
            Some(session) if session.status == UploadStatus::Open => {
                UploadError::PartBudgetExhausted {
                    limit: session.declared_size.saturating_mul(2),
                }
            },
            Some(session) => UploadError::Closed {
                status: session.status,
            },
            None => UploadError::NotFound,
        })
    }
}
