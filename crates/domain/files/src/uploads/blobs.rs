//! The blob reads an upload's owner may make: the bytes of a blob they
//! uploaded, for an import, and the stream of a completed upload, for a
//! download. An import reads a blob whole, after checking it passed the
//! content check its format needs; a blob past the limit is refused, never
//! truncated.
//!
//! TRACKED(phase-6 streaming import): the import parses a blob whole, so
//! `files.documents.maxUploadBytes` bounds it in memory; a streaming codec
//! import lifts this bound.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ContentHash, DocumentUploadId, UserId};
use systemprompt_traits::{ByteRange, StoredFileRead};

use systemprompt_models::documents::DocumentFormat;

use super::error::{RefusalReason, UploadError, UploadResult};
use super::models::{BlobOrigin, UploadSession, UploadStatus};
use super::safety::{Subject, inspect};
use super::service::UploadService;
use crate::documents::blob_store::BlobStoreError;

impl UploadService {
    pub async fn require_owned_blob(
        &self,
        owner: &UserId,
        blob: &ContentHash,
    ) -> UploadResult<BlobOrigin> {
        self.uploads
            .find_blob_origin(owner, blob)
            .await?
            .ok_or_else(|| UploadError::BlobNotOwned(blob.clone()))
    }

    pub async fn read_owned_blob(
        &self,
        owner: &UserId,
        blob: &ContentHash,
        format: DocumentFormat,
        max_bytes: u64,
    ) -> UploadResult<(Vec<u8>, BlobOrigin)> {
        let origin = self.require_owned_blob(owner, blob).await?;
        let origin = if origin.checked_kind.is_none() {
            self.recheck(owner, blob, origin).await?
        } else {
            origin
        };
        if !origin.admits(format) {
            return Err(UploadError::refused(
                RefusalReason::UncheckedFormat,
                format!(
                    "blob {blob} did not pass the content check a {format} document needs (it \
                     passed {}); import it as a file, or name it for what it is",
                    origin.checked_kind.as_deref().unwrap_or("none")
                ),
            ));
        }
        match self.blobs.read_all_primary(blob, max_bytes).await {
            Err(BlobStoreError::TooLarge { size, limit }) => {
                Err(UploadError::TooLarge { size, limit })
            },
            other => Ok((other?, origin)),
        }
    }

    async fn recheck(
        &self,
        owner: &UserId,
        blob: &ContentHash,
        origin: BlobOrigin,
    ) -> UploadResult<BlobOrigin> {
        let (id, size) = self.blobs.located(blob).await?;
        let subject = Subject {
            storage: self.storage.as_ref(),
            id: &id,
            size,
            declared_mime: &origin.mime,
        };
        let checked = inspect(&subject, &self.limits).await?.checked.as_str();
        self.uploads.set_checked_kind(owner, blob, checked).await?;
        Ok(BlobOrigin {
            checked_kind: Some(checked.to_owned()),
            ..origin
        })
    }

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
