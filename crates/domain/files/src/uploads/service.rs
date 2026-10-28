//! [`UploadService`]: opening a session, storing parts and progress. Abort
//! and expiry are in `lifecycle`, completion in `complete`, the blob reads
//! an upload's owner may make in `blobs`, and ingesting outside bytes in
//! `ingest`.
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
use systemprompt_traits::{FileByteStream, FileStorage, StoredFileId};

use super::error::{UploadError, UploadResult};
use super::models::{NewUpload, UploadPart, UploadProgress, UploadSession, UploadStatus};
use super::repository::{Admission, DocumentUploadRepository, SessionTerms};
use super::safety::sniff::declared_kind;
use super::stream::{Tally, part_storage_id};
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
        self.begin(owner, request, trace, false).await
    }

    /// Opens a session; a `one_shot` one (opened and finished in one call)
    /// is exempt from the open-session cap but not from the daily quota.
    pub(super) async fn begin(
        &self,
        owner: &UserId,
        request: NewUpload,
        trace: &TraceId,
        one_shot: bool,
    ) -> UploadResult<UploadSession> {
        let request = NewUpload {
            mime: request.mime.trim().to_ascii_lowercase(),
            ..request
        };
        self.admit(&request)?;
        let part_size = i64::try_from(self.limits.part_size_bytes)
            .map_err(|_| UploadError::Invalid("part size is out of range".to_owned()))?;
        let ttl_hours = i32::try_from(self.limits.session_ttl_hours).unwrap_or(i32::MAX);
        let terms = SessionTerms {
            owner,
            part_size,
            ttl_hours,
            trace,
            max_open: i64::try_from(self.limits.max_open_sessions).unwrap_or(i64::MAX),
            daily_quota: i64::try_from(self.limits.daily_quota_bytes).unwrap_or(i64::MAX),
            one_shot,
        };
        let session = match self.uploads.insert_admitted(&request, terms).await? {
            Admission::Admitted(session) => session,
            Admission::TooManyOpen { open } => {
                return Err(UploadError::TooManyOpen {
                    open,
                    limit: self.limits.max_open_sessions,
                });
            },
            Admission::QuotaExceeded { used } => {
                return Err(UploadError::QuotaExceeded {
                    used,
                    requested: request.size,
                    limit: self.limits.daily_quota_bytes,
                });
            },
        };
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

    fn admit(&self, request: &NewUpload) -> UploadResult<()> {
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
                "{} is not a MIME type",
                request.mime
            )));
        }
        if request.size > self.max_upload_bytes {
            return Err(UploadError::TooLarge {
                size: request.size,
                limit: self.max_upload_bytes,
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
        if !self.uploads.reserve_part_bytes(id, expected).await? {
            return Err(self.part_refusal(id).await?);
        }
        let attempt = part_storage_id(id.as_str(), part);
        let stored = self.store_part(id, part, expected, &attempt, body).await;
        if stored.is_err()
            && let Err(refund) = self.uploads.refund_part_bytes(id, expected).await
        {
            tracing::warn!(upload_id = %id, error = %refund, "Failed to refund a part attempt");
        }
        stored
    }

    /// Stores one part attempt under its own object and points the part's
    /// row at it; a failed attempt deletes only its own object, and the part
    /// it replaces is deleted only once the row no longer names it.
    async fn store_part(
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
    async fn part_refusal(&self, id: &DocumentUploadId) -> UploadResult<UploadError> {
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
