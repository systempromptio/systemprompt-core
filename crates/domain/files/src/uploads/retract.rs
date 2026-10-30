//! Retracting a completed upload whose caller never received its `blob_ref`,
//! so the blob does not wait out the garbage-collection grace period.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentUploadId, UserId};
use systemprompt_traits::StoredFileId;

use super::error::UploadResult;
use super::models::UploadStatus;
use super::service::UploadService;

impl UploadService {
    pub async fn retract(&self, owner: &UserId, id: &DocumentUploadId) -> UploadResult<()> {
        let session = self.owned(owner, id).await?;
        let Some(blob) = session
            .blob_sha256
            .clone()
            .filter(|_| session.status == UploadStatus::Completed)
        else {
            return Ok(());
        };
        if !self
            .uploads
            .mark_closed(id, UploadStatus::Completed, UploadStatus::Aborted, None)
            .await?
        {
            return Ok(());
        }
        if let Some(object) = self.uploads.withdraw_blob(&blob).await? {
            self.discard(id, &StoredFileId::new(object)).await?;
        }
        tracing::info!(upload_id = %id, user_id = %owner, blob = %blob, "Upload retracted");
        Ok(())
    }
}
