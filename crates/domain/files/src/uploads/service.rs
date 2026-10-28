//! [`UploadService`]: opening a session, storing parts, progress, abort and
//! expiry. Completion is in `complete`, and the blob reads an upload's
//! owner may make are in `blobs`.
//!
//! Every call names the caller; a session another user opened answers
//! [`UploadError::NotFound`], never its existence. Each lifecycle step is
//! logged with the session's trace id, and the session row keeps the
//! outcome, so the row and the log together are the upload's audit trail.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::Utc;
use systemprompt_identifiers::{DocumentUploadId, TraceId, UserId};
use systemprompt_traits::{FileByteStream, FileStorage, FileStorageError, StoredFileId};

use super::error::{UploadError, UploadResult};
use super::models::{NewUpload, UploadPart, UploadProgress, UploadSession, UploadStatus};
use super::repository::{DocumentUploadRepository, SessionTerms};
use super::safety::sniff::declared_kind;
use super::stream::{Tally, assembled_storage_id, part_storage_id};
use crate::config::UploadLimits;
use crate::documents::blob_store::BlobStore;

#[derive(Clone)]
pub struct UploadService {
    pub(super) storage: Arc<dyn FileStorage>,
    pub(super) uploads: Arc<DocumentUploadRepository>,
    pub(super) blobs: Arc<BlobStore>,
    pub(super) max_upload_bytes: u64,
    pub(super) limits: UploadLimits,
}

impl std::fmt::Debug for UploadService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadService")
            .field("max_upload_bytes", &self.max_upload_bytes)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

/// The handles an [`UploadService`] is built from.
#[derive(Clone)]
pub struct UploadServiceParts {
    pub storage: Arc<dyn FileStorage>,
    pub uploads: Arc<DocumentUploadRepository>,
    pub blobs: Arc<BlobStore>,
    pub max_upload_bytes: u64,
    pub limits: UploadLimits,
}

impl std::fmt::Debug for UploadServiceParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadServiceParts").finish_non_exhaustive()
    }
}

impl UploadService {
    #[must_use]
    pub fn new(parts: UploadServiceParts) -> Self {
        Self {
            storage: parts.storage,
            uploads: parts.uploads,
            blobs: parts.blobs,
            max_upload_bytes: parts.max_upload_bytes,
            limits: parts.limits,
        }
    }

    #[must_use]
    pub const fn limits(&self) -> &UploadLimits {
        &self.limits
    }

    #[must_use]
    pub const fn max_upload_bytes(&self) -> u64 {
        self.max_upload_bytes
    }

    pub async fn create(
        &self,
        owner: &UserId,
        request: NewUpload,
        trace: &TraceId,
    ) -> UploadResult<UploadSession> {
        self.admit(owner, &request).await?;
        let part_size = i64::try_from(self.limits.part_size_bytes)
            .map_err(|_| UploadError::Invalid("part size is out of range".to_owned()))?;
        let ttl_hours = i32::try_from(self.limits.session_ttl_hours).unwrap_or(i32::MAX);
        let terms = SessionTerms {
            owner,
            part_size,
            ttl_hours,
            trace,
        };
        let session = self.uploads.insert(&request, terms).await?;
        tracing::info!(
            upload_id = %session.id,
            user_id = %owner,
            trace_id = %trace,
            mime = %session.declared_mime,
            size = session.declared_size,
            parts = session.part_count(),
            "Upload session opened"
        );
        Ok(session)
    }

    async fn admit(&self, owner: &UserId, request: &NewUpload) -> UploadResult<()> {
        if request.name.trim().is_empty() || request.name.contains(['/', '\\', '\0']) {
            return Err(UploadError::Invalid(
                "name must be a non-empty file name without path separators".to_owned(),
            ));
        }
        if request.size == 0 {
            return Err(UploadError::Invalid(
                "size must be at least one byte".to_owned(),
            ));
        }
        if declared_kind(&request.mime).is_none() {
            return Err(UploadError::Invalid(format!(
                "{} uploads are not accepted",
                request.mime
            )));
        }
        if request.size > self.max_upload_bytes {
            return Err(UploadError::TooLarge {
                size: request.size,
                limit: self.max_upload_bytes,
            });
        }
        let open = self.uploads.count_open(owner).await?.unsigned_abs();
        if open >= self.limits.max_open_sessions {
            return Err(UploadError::TooManyOpen {
                open,
                limit: self.limits.max_open_sessions,
            });
        }
        let used = self.uploads.bytes_last_day(owner).await?.unsigned_abs();
        if used.saturating_add(request.size) > self.limits.daily_quota_bytes {
            return Err(UploadError::QuotaExceeded {
                used,
                requested: request.size,
                limit: self.limits.daily_quota_bytes,
            });
        }
        Ok(())
    }

    pub(super) async fn owned(
        &self,
        owner: &UserId,
        id: &DocumentUploadId,
    ) -> UploadResult<UploadSession> {
        match self.uploads.find(id).await? {
            Some(session) if session.owner_user_id == *owner => Ok(session),
            _ => Err(UploadError::NotFound),
        }
    }

    pub(super) async fn open_session(
        &self,
        owner: &UserId,
        id: &DocumentUploadId,
    ) -> UploadResult<UploadSession> {
        let session = self.owned(owner, id).await?;
        if session.status != UploadStatus::Open {
            return Err(UploadError::Closed {
                status: session.status,
            });
        }
        if session.expires_at <= Utc::now() {
            return Err(UploadError::Closed {
                status: UploadStatus::Expired,
            });
        }
        Ok(session)
    }

    pub async fn progress(
        &self,
        owner: &UserId,
        id: &DocumentUploadId,
    ) -> UploadResult<UploadProgress> {
        let session = self.owned(owner, id).await?;
        let parts = self.uploads.list_parts(id).await?;
        Ok(UploadProgress { session, parts })
    }

    pub async fn put_part(
        &self,
        owner: &UserId,
        id: &DocumentUploadId,
        part: u32,
        body: FileByteStream,
    ) -> UploadResult<UploadPart> {
        let session = self.open_session(owner, id).await?;
        let parts = session.part_count();
        if part == 0 || part > parts {
            return Err(UploadError::PartOutOfRange { part, parts });
        }
        let expected = session.expected_part_size(part);
        let storage_id = part_storage_id(id.as_str(), part);
        let tally = Tally::new(expected);
        let stored = self
            .storage
            .store_stream(std::path::Path::new(storage_id.as_str()), tally.wrap(body))
            .await;
        let actual = tally.len();
        if tally.overflowed() || (stored.is_ok() && actual != expected) {
            self.discard(&storage_id).await;
            return Err(UploadError::PartSize {
                part,
                expected,
                actual,
            });
        }
        stored?;
        let sha256 = tally
            .hash()
            .ok_or_else(|| UploadError::Invalid("part hash could not be formed".to_owned()))?;
        let stored_part = UploadPart {
            part_number: part,
            size_bytes: actual,
            sha256,
            storage_id: storage_id.as_str().to_owned(),
        };
        self.uploads.upsert_part(id, &stored_part).await?;
        Ok(stored_part)
    }

    pub async fn abort(&self, owner: &UserId, id: &DocumentUploadId) -> UploadResult<()> {
        let session = self.open_session(owner, id).await?;
        if self
            .uploads
            .mark_closed(id, UploadStatus::Aborted, None)
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
                .mark_closed(&id, UploadStatus::Expired, None)
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
