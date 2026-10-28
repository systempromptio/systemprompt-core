//! The blob reads an upload's owner may make: the bytes of a blob they
//! uploaded, for an import, and the stream of a completed upload, for a
//! download.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ContentHash, DocumentUploadId, UserId};
use systemprompt_traits::{ByteRange, StoredFileRead};

use super::error::{UploadError, UploadResult};
use super::models::{UploadSession, UploadStatus};
use super::service::UploadService;
use crate::documents::blob_store::BlobStoreError;

impl UploadService {
    /// Refuses unless `owner` completed an upload that produced `blob`, so a
    /// content hash alone never opens bytes someone else uploaded.
    pub async fn require_owned_blob(&self, owner: &UserId, blob: &ContentHash) -> UploadResult<()> {
        if self.uploads.owns_blob(owner, blob).await? {
            Ok(())
        } else {
            Err(UploadError::BlobNotOwned(blob.clone()))
        }
    }

    /// The bytes of a blob `owner` uploaded, whole; a blob past `max_bytes`
    /// is refused, never truncated.
    pub async fn read_owned_blob(
        &self,
        owner: &UserId,
        blob: &ContentHash,
        max_bytes: u64,
    ) -> UploadResult<Vec<u8>> {
        self.require_owned_blob(owner, blob).await?;
        match self.blobs.read_all_primary(blob, max_bytes).await {
            Err(BlobStoreError::TooLarge { size, limit }) => {
                Err(UploadError::TooLarge { size, limit })
            },
            other => Ok(other?),
        }
    }

    /// Streams the blob a completed upload produced, optionally one range.
    pub async fn open_blob(
        &self,
        owner: &UserId,
        id: &DocumentUploadId,
        range: Option<ByteRange>,
    ) -> UploadResult<(UploadSession, StoredFileRead)> {
        let session = self.owned(owner, id).await?;
        let blob = match (&session.status, &session.blob_sha256) {
            (UploadStatus::Completed, Some(blob)) => blob.clone(),
            (status, _) => return Err(UploadError::Closed { status: *status }),
        };
        let read = self.blobs.open(&blob, range).await?;
        Ok((session, read))
    }
}
