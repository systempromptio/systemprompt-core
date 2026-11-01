//! Closing an upload without completing it: abort by its owner, expiry by
//! the job, deleting the objects a closed upload stored, and sweeping the
//! objects an interrupted attempt or a failed delete left behind.
//!
//! A delete that fails is never dropped: the object is recorded as an
//! orphan of its upload ([`DocumentUploadRepository::track_object`]) and
//! the caller is told how many were kept ([`ObjectRemoval`]), so the next
//! sweep retries it.
//!
//! [`DocumentUploadRepository::track_object`]: super::repository::DocumentUploadRepository::track_object
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentUploadId, UserId};
use systemprompt_traits::{FileStorageError, StoredFileId};

use super::error::UploadResult;
use super::models::{UploadSession, UploadStatus};
use super::service::UploadService;
use super::stream::assembled_storage_id;

/// Whether one upload object was deleted or kept for the sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Discarded {
    Deleted,
    Retained,
}

/// The objects a closed upload's removal deleted and the ones it kept for
/// the sweep after their delete failed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectRemoval {
    pub deleted: u64,
    pub retained: Vec<StoredFileId>,
}

impl ObjectRemoval {
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.retained.is_empty()
    }

    fn count(&mut self, object: StoredFileId, discarded: Discarded) {
        match discarded {
            Discarded::Deleted => self.deleted += 1,
            Discarded::Retained => self.retained.push(object),
        }
    }
}

/// What one expiry pass closed and what its removals kept.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExpiryOutcome {
    pub expired: u64,
    pub retained: u64,
}

/// What one orphan sweep deleted and what it left for the next pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SweepOutcome {
    pub deleted: u64,
    pub retained: u64,
}

impl UploadService {
    pub async fn abort(
        &self,
        owner: &UserId,
        id: &DocumentUploadId,
    ) -> UploadResult<ObjectRemoval> {
        let session = self.open_session(owner, id).await?;
        if !self
            .uploads
            .mark_closed(id, UploadStatus::Open, UploadStatus::Aborted, None)
            .await?
        {
            return Ok(ObjectRemoval::default());
        }
        let removal = self.remove_objects(&session).await?;
        tracing::info!(
            upload_id = %id,
            user_id = %owner,
            trace_id = %session.trace_id,
            retained = removal.retained.len(),
            "Upload aborted"
        );
        Ok(removal)
    }

    pub async fn expire(&self, limit: i64) -> UploadResult<ExpiryOutcome> {
        let mut outcome = ExpiryOutcome::default();
        for id in self.uploads.list_expired(limit).await? {
            let Some(session) = self.uploads.find(&id).await? else {
                continue;
            };
            if self
                .uploads
                .mark_closed(&id, session.status, UploadStatus::Expired, None)
                .await?
            {
                let removal = self.remove_objects(&session).await?;
                let retained = u64::try_from(removal.retained.len()).unwrap_or(u64::MAX);
                outcome.expired += 1;
                outcome.retained = outcome.retained.saturating_add(retained);
                tracing::info!(
                    upload_id = %id,
                    trace_id = %session.trace_id,
                    retained = removal.retained.len(),
                    "Upload expired"
                );
            }
        }
        Ok(outcome)
    }

    pub async fn sweep_orphans(&self, limit: i64) -> UploadResult<SweepOutcome> {
        let mut outcome = SweepOutcome::default();
        for orphan in self.uploads.list_orphaned_objects(limit).await? {
            match self.discard(&orphan.upload_id, &orphan.storage_id).await? {
                Discarded::Deleted => outcome.deleted += 1,
                Discarded::Retained => outcome.retained += 1,
            }
        }
        Ok(outcome)
    }

    pub(super) async fn remove_objects(
        &self,
        session: &UploadSession,
    ) -> UploadResult<ObjectRemoval> {
        let mut removal = ObjectRemoval::default();
        for part in self.uploads.list_parts(&session.id).await? {
            let object = StoredFileId::new(part.storage_id.as_str());
            let discarded = self.discard(&session.id, &object).await?;
            removal.count(object, discarded);
        }
        let assembled = assembled_storage_id(session.id.as_str());
        let adopted = self
            .blobs
            .storage_id_of(&session.declared_sha256)
            .await?
            .is_some_and(|stored| stored == assembled);
        if !adopted && self.storage.exists(&assembled).await? {
            let discarded = self.discard(&session.id, &assembled).await?;
            removal.count(assembled, discarded);
        }
        Ok(removal)
    }

    pub(super) async fn discard(
        &self,
        upload: &DocumentUploadId,
        object: &StoredFileId,
    ) -> UploadResult<Discarded> {
        match self.storage.delete(object).await {
            Ok(()) | Err(FileStorageError::NotFound(_)) => {
                self.uploads.untrack_object(object).await?;
                Ok(Discarded::Deleted)
            },
            Err(error) => {
                tracing::warn!(
                    upload_id = %upload,
                    storage_id = %object,
                    error = %error,
                    "Upload object delete failed; kept for the orphan sweep"
                );
                self.uploads.track_object(upload, object).await?;
                Ok(Discarded::Retained)
            },
        }
    }
}
