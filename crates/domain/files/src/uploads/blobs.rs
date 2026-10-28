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
    /// The MIME type `owner`'s upload recorded for `blob`; refuses unless
    /// `owner` completed an upload that produced it, so a content hash alone
    /// never opens bytes someone else uploaded.
    pub async fn require_owned_blob(
        &self,
        owner: &UserId,
        blob: &ContentHash,
    ) -> UploadResult<String> {
        self.uploads
            .find_blob_mime(owner, blob)
            .await?
            .ok_or_else(|| UploadError::BlobNotOwned(blob.clone()))
    }

    /// The bytes of a blob `owner` uploaded, whole, and its MIME type; a blob
    /// past `max_bytes` is refused, never truncated.
    // TRACKED(phase-6 streaming import): the import parses a blob whole, so
    // `max_bytes` (files.documents.maxUploadBytes) bounds it in memory; a
    // streaming codec import lifts this bound.
    pub async fn read_owned_blob(
        &self,
        owner: &UserId,
        blob: &ContentHash,
        max_bytes: u64,
    ) -> UploadResult<(Vec<u8>, String)> {
        let mime = self.require_owned_blob(owner, blob).await?;
        match self.blobs.read_all_primary(blob, max_bytes).await {
            Err(BlobStoreError::TooLarge { size, limit }) => {
                Err(UploadError::TooLarge { size, limit })
            },
            other => Ok((other?, mime)),
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
