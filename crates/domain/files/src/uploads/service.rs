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
use systemprompt_traits::{BackgroundTasks, FileByteStream, FileStorage};

use super::attempts::PartReservation;
use super::error::{UploadError, UploadResult};
use super::models::{
    Allowance, NewUpload, UploadPart, UploadProgress, UploadSession, UploadStatus,
};
use super::repository::{Admission, DocumentUploadRepository, SessionTerms};
use super::safety::sniff::declared_kind;
use super::stream::part_storage_id;
use crate::config::UploadLimits;
use crate::documents::blob_store::BlobStore;

#[derive(Clone)]
pub struct UploadService {
    pub(super) storage: Arc<dyn FileStorage>,
    pub(super) uploads: Arc<DocumentUploadRepository>,
    pub(super) blobs: Arc<BlobStore>,
    pub(super) max_upload_bytes: u64,
    pub(super) limits: UploadLimits,
    pub(super) tasks: BackgroundTasks,
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
    pub tasks: BackgroundTasks,
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
            tasks: parts.tasks,
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
        self.begin(owner, request, trace, Allowance::Session).await
    }

    pub(super) async fn begin(
        &self,
        owner: &UserId,
        request: NewUpload,
        trace: &TraceId,
        allowance: Allowance,
    ) -> UploadResult<UploadSession> {
        let request = NewUpload {
            mime: request.mime.trim().to_ascii_lowercase(),
            ..request
        };
        self.admit(&request)?;
        let part_size = i64::try_from(self.limits.part_size_bytes).map_err(|_out_of_range| {
            UploadError::Invalid("part size is out of range".to_owned())
        })?;
        let ttl_hours = i32::try_from(self.limits.session_ttl_hours).unwrap_or(i32::MAX);
        let terms = SessionTerms {
            owner,
            part_size,
            ttl_hours,
            trace,
            max_open: i64::try_from(self.limits.max_open_sessions).unwrap_or(i64::MAX),
            daily_quota: i64::try_from(self.limits.daily_quota_bytes).unwrap_or(i64::MAX),
            one_shot: allowance.one_shot(),
            quota_user: allowance.charged_to(owner),
            new_bytes_only: allowance.new_bytes_only(),
        };
        let session = match self.uploads.insert_admitted(&request, terms).await? {
            Admission::Admitted(session) => *session,
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
        let mut reservation = PartReservation::new(
            Arc::clone(&self.uploads),
            self.tasks.clone(),
            id.clone(),
            expected,
        );
        let attempt = part_storage_id(id.as_str(), part);
        let stored = self
            .store_part(id, part, expected, &attempt, body, &mut reservation)
            .await;
        if stored.is_err() {
            reservation.refund().await;
        }
        stored
    }
}
