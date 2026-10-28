//! The entry points for document bytes that arrive from outside the upload
//! API: a bridge drive commit and a REST whole-file write. Each runs the
//! bytes through the attached [`UploadService`]'s checks
//! ([`UploadService::ingest`]) before any codec parses them, so no path into
//! the codecs skips the sniff and zip guard. Bytes the store produces itself
//! (templates, restores, live checkpoints) take [`DocumentService::import`]
//! and [`DocumentService::commit`] directly.
//!
//! A new document's import counts toward the daily upload quota; a save of
//! an existing document runs every check but does not spend it
//! ([`Allowance::Save`]).
//!
//! With no upload service attached, external bytes are refused rather than
//! parsed unchecked.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, DocumentFormat};

use super::support::format_of_name;
use super::{CommitOutcome, CommitRequest, DocumentService, ImportRequest};
use crate::documents::blob_store::BlobStoreError;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision};
use crate::documents::revision::CommitBody;
use crate::uploads::{Allowance, UploadError, UploadService};

fn refused(error: UploadError) -> DocumentsError {
    match error {
        UploadError::TooLarge { size, limit } => DocumentsError::TooLarge {
            what: "upload",
            size,
            limit,
        },
        UploadError::Storage(error) => DocumentsError::Blob(BlobStoreError::Storage(error)),
        UploadError::Blob(error) => DocumentsError::Blob(error),
        UploadError::Repository(error) => DocumentsError::Repository(error),
        UploadError::Interrupted(error) => DocumentsError::Worker(error),
        other => DocumentsError::ContentRefused {
            code: other.code(),
            detail: other.to_string(),
        },
    }
}

impl DocumentService {
    pub fn attach_uploads(&self, uploads: &Arc<UploadService>) {
        if self.uploads.set(Arc::clone(uploads)).is_err() {
            tracing::warn!("An upload service is already attached to the document service");
        }
    }

    async fn ingest(
        &self,
        who: &AuthenticatedUser,
        name: &str,
        format: DocumentFormat,
        bytes: Vec<u8>,
        trace: &TraceId,
        allowance: Allowance,
    ) -> Result<(Vec<u8>, Option<String>), DocumentsError> {
        if bytes.is_empty() {
            return Ok((bytes, None));
        }
        let uploads = self.uploads.get().ok_or(DocumentsError::ContentRefused {
            code: "uploads_unavailable",
            detail: "no upload service is attached to check these bytes".to_owned(),
        })?;
        let ingested = uploads
            .ingest(&who.id, name, format, bytes, trace, allowance)
            .await
            .map_err(refused)?;
        Ok((ingested.bytes, Some(ingested.mime)))
    }

    /// [`DocumentService::import`] for bytes from outside the upload API.
    pub async fn import_external(
        &self,
        who: &AuthenticatedUser,
        mut request: ImportRequest,
        trace: &TraceId,
    ) -> Result<(Document, DocumentRevision), DocumentsError> {
        self.check_size(request.bytes.len())?;
        let format = request
            .format
            .or_else(|| format_of_name(&request.name))
            .unwrap_or(DocumentFormat::File);
        self.authorize_create(who, request.folder.as_ref(), trace)
            .await?;
        let bytes = std::mem::take(&mut request.bytes);
        let (bytes, mime) = self
            .ingest(who, &request.name, format, bytes, trace, Allowance::OneShot)
            .await?;
        request.bytes = bytes;
        request.format = Some(format);
        if mime.is_some() {
            request.mime_type = mime;
        }
        self.import(who, request, trace).await
    }

    /// [`DocumentService::commit`] for a whole-file write from outside the
    /// upload API; an ops commit passes straight through.
    pub async fn commit_external(
        &self,
        who: &AuthenticatedUser,
        mut request: CommitRequest,
        trace: &TraceId,
    ) -> Result<CommitOutcome, DocumentsError> {
        if let CommitBody::Bytes(bytes) = &mut request.body {
            let document = self
                .authorized_document(who, &request.document_id, DocumentAction::Edit, trace)
                .await?;
            self.check_size(bytes.len())?;
            let (checked, _) = self
                .ingest(
                    who,
                    &document.name,
                    document.format,
                    std::mem::take(bytes),
                    trace,
                )
                .await?;
            *bytes = checked;
        }
        self.commit(who, request, trace).await
    }
}
