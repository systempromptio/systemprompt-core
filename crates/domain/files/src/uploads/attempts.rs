//! One part attempt: recorded, stored under its own object, then swapped
//! into the part's row, and why a part was refused when it was not taken.
//!
//! The attempt's byte reservation is held by a [`PartReservation`] for as
//! long as the attempt runs. An attempt that fails refunds it; one whose
//! future is dropped (a timeout, a closed connection, shutdown) refunds it
//! from an owned background task, so no exit leaves bytes reserved. Once the
//! swap is under way the refund names the attempt's object and is skipped if
//! the swap committed, so a future dropped mid-commit never refunds a part
//! that landed. The
//! attempt's object is recorded before its bytes are stored, so an exit
//! between the store and the swap leaves an object the orphan sweep finds.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::DocumentUploadId;
use systemprompt_traits::{BackgroundTasks, FileByteStream, StoredFileId};

use super::error::{UploadError, UploadResult};
use super::models::{UploadPart, UploadStatus};
use super::repository::DocumentUploadRepository;
use super::service::UploadService;
use super::stream::Tally;

#[derive(Debug)]
pub(super) struct PartReservation {
    uploads: Arc<DocumentUploadRepository>,
    tasks: BackgroundTasks,
    upload: DocumentUploadId,
    attempt: Option<StoredFileId>,
    bytes: u64,
    armed: bool,
}

impl PartReservation {
    pub(super) const fn new(
        uploads: Arc<DocumentUploadRepository>,
        tasks: BackgroundTasks,
        upload: DocumentUploadId,
        bytes: u64,
    ) -> Self {
        Self {
            uploads,
            tasks,
            upload,
            attempt: None,
            bytes,
            armed: true,
        }
    }

    pub(super) const fn keep(&mut self) {
        self.armed = false;
    }

    fn landing(&mut self, attempt: &StoredFileId) {
        self.attempt = Some(attempt.clone());
    }

    pub(super) async fn refund(mut self) {
        if std::mem::replace(&mut self.armed, false) {
            refund(
                &self.uploads,
                &self.upload,
                self.attempt.as_ref(),
                self.bytes,
            )
            .await;
        }
    }
}

impl Drop for PartReservation {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let uploads = Arc::clone(&self.uploads);
        let upload = self.upload.clone();
        let attempt = self.attempt.take();
        let bytes = self.bytes;
        self.tasks.spawn("upload_part_refund", async move {
            refund(&uploads, &upload, attempt.as_ref(), bytes).await;
        });
    }
}

async fn refund(
    uploads: &DocumentUploadRepository,
    upload: &DocumentUploadId,
    attempt: Option<&StoredFileId>,
    bytes: u64,
) {
    if let Err(error) = uploads.refund_part_bytes(upload, attempt, bytes).await {
        tracing::error!(
            upload_id = %upload,
            bytes,
            error = %error,
            "Failed to refund a part attempt's reserved bytes"
        );
    }
}

impl UploadService {
    #[expect(
        clippy::too_many_arguments,
        reason = "one part attempt is named by its upload, part number, expected size, object, body and reservation"
    )]
    pub(super) async fn store_part(
        &self,
        id: &DocumentUploadId,
        part: u32,
        expected: u64,
        attempt: &StoredFileId,
        body: FileByteStream,
        reservation: &mut PartReservation,
    ) -> UploadResult<UploadPart> {
        self.uploads.track_object(id, attempt).await?;
        let tally = Tally::new(expected);
        let stored = self
            .storage
            .store_stream(std::path::Path::new(attempt.as_str()), tally.wrap(body))
            .await;
        let actual = tally.len();
        if tally.overflowed() || (stored.is_ok() && actual != expected) {
            self.discard(id, attempt).await?;
            return Err(UploadError::PartSize {
                part,
                expected,
                actual,
            });
        }
        if let Err(error) = stored {
            self.discard(id, attempt).await?;
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
        reservation.landing(attempt);
        let swap = self.uploads.swap_part(id, &stored_part).await?;
        if !swap.written {
            self.discard(id, attempt).await?;
            return Err(self.part_refusal(id).await?);
        }
        reservation.keep();
        self.uploads.untrack_object(attempt).await?;
        if let Some(previous) = swap.previous {
            self.discard(id, &StoredFileId::new(previous)).await?;
        }
        Ok(stored_part)
    }

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
