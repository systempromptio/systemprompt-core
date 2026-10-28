//! Completing an upload: assembling its parts, verifying the declared
//! SHA-256, running the content checks and registering the result as a
//! content-addressed blob; and [`UploadService::put_inline`], the one-call
//! upload of a small body.
//!
//! A completion first claims the upload (`open` -> `completing`), so an
//! abort or the expiry job cannot close it mid-assembly; a failure the
//! caller may retry hands it back to `open`. If the claim is lost anyway (a
//! crashed completion closed by the expiry job), a blob row this completion
//! created is withdrawn again rather than left without its upload.
//!
//! The parts are streamed, in order, into one object while being hashed.
//! A hash that differs from the declared one, or content the checks refuse,
//! closes the session as `refused` and deletes everything it stored. Bytes
//! already stored under the same hash are reused: the assembled copy is
//! dropped and the upload answers the existing blob (`deduplicated`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use systemprompt_identifiers::{DocumentUploadId, TraceId, UserId};
use systemprompt_traits::FileStorageError;

use super::error::{RefusalReason, UploadError, UploadResult};
use super::models::{CompletedUpload, NewUpload, UploadSession, UploadStatus};
use super::safety::{Subject, inspect};
use super::service::UploadService;
use super::stream::{Tally, assembled_storage_id, concatenated};

impl UploadService {
    pub async fn complete(
        &self,
        owner: &UserId,
        id: &DocumentUploadId,
    ) -> UploadResult<CompletedUpload> {
        let session = self.owned(owner, id).await?;
        if session.status == UploadStatus::Completed
            && let Some(blob) = session.blob_sha256.clone()
        {
            return Ok(CompletedUpload {
                blob,
                size_bytes: session.declared_size,
                detected_mime: session.detected_mime.clone().unwrap_or_default(),
                deduplicated: true,
                session,
            });
        }
        let session = self.open_session(owner, id).await?;
        if !self.uploads.claim(id).await? {
            let status = self
                .uploads
                .find(id)
                .await?
                .map_or(UploadStatus::Expired, |current| current.status);
            return Err(UploadError::Closed { status });
        }
        match self.finish(&session).await {
            Err(UploadError::Refused { reason, detail }) => {
                self.refuse(&session, reason, &detail).await?;
                Err(UploadError::Refused { reason, detail })
            },
            Err(error) => {
                if let Err(release) = self.uploads.release(id).await {
                    tracing::warn!(upload_id = %id, error = %release, "Failed to release a claimed upload");
                }
                Err(error)
            },
            ok => ok,
        }
    }

    async fn finish(&self, session: &UploadSession) -> UploadResult<CompletedUpload> {
        let parts = self.uploads.list_parts(&session.id).await?;
        let progress = super::models::UploadProgress {
            session: session.clone(),
            parts,
        };
        let missing = progress.missing_parts();
        if !missing.is_empty() {
            return Err(UploadError::MissingParts { missing });
        }
        let assembled = assembled_storage_id(session.id.as_str());
        let tally = Tally::new(session.declared_size);
        let sources = progress
            .parts
            .iter()
            .map(|part| part.storage_id.as_str().into())
            .collect();
        self.storage
            .store_stream(
                Path::new(assembled.as_str()),
                tally.wrap(concatenated(Arc::clone(&self.storage), sources)),
            )
            .await?;
        let actual = tally.hash();
        if actual.as_ref() != Some(&session.declared_sha256) {
            return Err(UploadError::refused(
                RefusalReason::ChecksumMismatch,
                format!(
                    "declared sha256 {} but the parts hash to {}",
                    session.declared_sha256,
                    actual.map_or_else(|| "nothing".to_owned(), |hash| hash.to_string())
                ),
            ));
        }
        let subject = Subject {
            storage: self.storage.as_ref(),
            id: &assembled,
            size: session.declared_size,
            declared_mime: &session.declared_mime,
        };
        let inspected = inspect(&subject, &self.limits).await?;
        let blob = match inspected.replacement {
            Some(bytes) => {
                let stored = self.blobs.put(&bytes).await?;
                self.discard(&assembled).await;
                stored
            },
            None => {
                self.blobs
                    .adopt(&session.declared_sha256, session.declared_size, &assembled)
                    .await?
            },
        };
        for part in &progress.parts {
            self.discard(&part.storage_id.as_str().into()).await;
        }
        let closed = self
            .uploads
            .mark_completed(
                &session.id,
                &blob.hash,
                &inspected.detected_mime,
                inspected.checked.as_str(),
            )
            .await?;
        if !closed {
            if blob.inserted
                && let Some(object) = self.uploads.withdraw_blob(&blob.hash).await?
            {
                self.discard(&object.as_str().into()).await;
            }
            return Err(UploadError::Closed {
                status: UploadStatus::Expired,
            });
        }
        tracing::info!(
            upload_id = %session.id,
            user_id = %session.owner_user_id,
            trace_id = %session.trace_id,
            blob = %blob.hash,
            size = blob.size_bytes,
            detected_mime = %inspected.detected_mime,
            deduplicated = !blob.inserted,
            "Upload completed"
        );
        Ok(CompletedUpload {
            session: session.clone(),
            blob: blob.hash,
            size_bytes: blob.size_bytes,
            detected_mime: inspected.detected_mime,
            deduplicated: !blob.inserted,
        })
    }

    async fn refuse(
        &self,
        session: &UploadSession,
        reason: RefusalReason,
        detail: &str,
    ) -> UploadResult<()> {
        let refusal = format!("{reason}: {detail}");
        if self
            .uploads
            .mark_closed(
                &session.id,
                UploadStatus::Completing,
                UploadStatus::Refused,
                Some(&refusal),
            )
            .await?
        {
            self.remove_objects(session).await?;
        }
        tracing::warn!(
            upload_id = %session.id,
            user_id = %session.owner_user_id,
            trace_id = %session.trace_id,
            reason = %reason,
            detail = %detail,
            "Upload refused"
        );
        Ok(())
    }

    /// Uploads a body small enough to send in one call, as one session.
    pub async fn put_inline(
        &self,
        owner: &UserId,
        request: NewUpload,
        bytes: Vec<u8>,
        trace: &TraceId,
    ) -> UploadResult<CompletedUpload> {
        let size = bytes.len() as u64;
        if size > self.limits.inline_max_bytes {
            return Err(UploadError::TooLarge {
                size,
                limit: self.limits.inline_max_bytes,
            });
        }
        if size != request.size {
            return Err(UploadError::Invalid(format!(
                "declared size {} but the body is {size} bytes",
                request.size
            )));
        }
        let session = self.create(owner, request, trace).await?;
        let stored = self.put_all(owner, &session, bytes).await;
        if let Err(error) = stored {
            if let Err(abort) = self.abort(owner, &session.id).await {
                tracing::warn!(upload_id = %session.id, error = %abort, "Failed to abort an inline upload");
            }
            return Err(error);
        }
        self.complete(owner, &session.id).await
    }

    async fn put_all(
        &self,
        owner: &UserId,
        session: &UploadSession,
        bytes: Vec<u8>,
    ) -> UploadResult<()> {
        let chunk = usize::try_from(session.part_size).unwrap_or(usize::MAX);
        let bytes = bytes::Bytes::from(bytes);
        for (index, part) in (1..).zip(bytes.chunks(chunk)) {
            let body = bytes.slice_ref(part);
            let stream = futures_util::stream::once(async move { Ok::<_, FileStorageError>(body) });
            self.put_part(owner, &session.id, index, Box::pin(stream))
                .await?;
        }
        Ok(())
    }
}
