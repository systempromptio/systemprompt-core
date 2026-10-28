//! Closing an upload without completing it: abort by its owner, expiry by
//! the job, and deleting the objects a closed upload stored.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentUploadId, UserId};
use systemprompt_traits::{FileStorageError, StoredFileId};

use super::error::UploadResult;
use super::models::{UploadSession, UploadStatus};
use super::service::UploadService;
use super::stream::assembled_storage_id;

impl UploadService {
    pub async fn abort(&self, owner: &UserId, id: &DocumentUploadId) -> UploadResult<()> {
        let session = self.open_session(owner, id).await?;
        if self
            .uploads
            .mark_closed(id, UploadStatus::Open, UploadStatus::Aborted, None)
            .await?
        {
            self.remove_objects(&session).await?;
            tracing::info!(upload_id = %id, user_id = %owner, trace_id = %session.trace_id, "Upload aborted");
        }
        Ok(())
    }

    /// Closes up to `limit` open sessions past their expiry and deletes
    /// their stored bytes; answers how many it closed.
    pub async fn expire(&self, limit: i64) -> UploadResult<u64> {
        let mut expired = 0;
        for id in self.uploads.list_expired(limit).await? {
            let Some(session) = self.uploads.find(&id).await? else {
                continue;
            };
            if self
                .uploads
                .mark_closed(&id, session.status, UploadStatus::Expired, None)
                .await?
            {
                self.remove_objects(&session).await?;
                expired += 1;
                tracing::info!(upload_id = %id, trace_id = %session.trace_id, "Upload expired");
            }
        }
        Ok(expired)
    }

    pub(super) async fn remove_objects(&self, session: &UploadSession) -> UploadResult<()> {
        for part in self.uploads.list_parts(&session.id).await? {
            self.discard(&StoredFileId::new(part.storage_id)).await;
        }
        let assembled = assembled_storage_id(session.id.as_str());
        let adopted = self
            .blobs
            .storage_id_of(&session.declared_sha256)
            .await?
            .is_some_and(|stored| stored == assembled);
        if !adopted && self.storage.exists(&assembled).await? {
            self.discard(&assembled).await;
        }
        Ok(())
    }

    pub(super) async fn discard(&self, id: &StoredFileId) {
        match self.storage.delete(id).await {
            Ok(()) | Err(FileStorageError::NotFound(_)) => {},
            Err(error) => {
                tracing::warn!(storage_id = %id, error = %error, "Failed to delete an upload object");
            },
        }
    }
}
