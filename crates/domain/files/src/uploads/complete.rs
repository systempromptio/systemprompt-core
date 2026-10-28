//! Completing an upload: assembling its parts, verifying the declared
//! SHA-256, running the content checks and registering the result as a
//! content-addressed blob. One-call uploads of a body held in memory are in
//! `whole`.
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
use std::time::Duration;

use systemprompt_identifiers::{DocumentUploadId, UserId};

use super::error::{RefusalReason, UploadError, UploadResult};
use super::models::{CompletedUpload, UploadSession, UploadStatus};
use super::safety::{Subject, inspect};
use super::service::UploadService;
use super::stream::{Tally, assembled_storage_id, concatenated};

/// How often a running completion refreshes its claim; the expiry job
/// closes a claim not refreshed for an hour.
const CLAIM_HEARTBEAT: Duration = Duration::from_secs(300);

/// Stops a completion's claim heartbeat however the completion ends (an
/// early error return or a panic included), so a dead completion's claim
/// goes stale and the expiry job can close it.
struct Heartbeat(tokio::task::JoinHandle<()>);

impl Drop for Heartbeat {
    fn drop(&mut self) {
        self.0.abort();
    }
}

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
                checked_kind: session.checked_kind.clone().unwrap_or_default(),
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
        let service = self.clone();
        let completion = tokio::spawn(async move { service.finish_claimed(session).await });
        completion.await?
    }

    /// Finishes a claimed upload on its own task, so a client that drops the
    /// `complete` request (a proxy timeout) does not cancel the work and
    /// strand the claim: the upload still completes and a retry answers it.
    /// A heartbeat keeps the claim fresh while a large assembly runs, so the
    /// expiry job only closes claims whose completion really died.
    async fn finish_claimed(self, session: UploadSession) -> UploadResult<CompletedUpload> {
        let _heartbeat = Heartbeat({
            let uploads = Arc::clone(&self.uploads);
            let id = session.id.clone();
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(CLAIM_HEARTBEAT);
                tick.tick().await;
                loop {
                    tick.tick().await;
                    if let Err(error) = uploads.touch_claim(&id).await {
                        tracing::warn!(upload_id = %id, error = %error, "Failed to refresh an upload claim");
                    }
                }
            })
        });
        let outcome = match self.finish(&session).await {
            Err(UploadError::Refused { reason, detail }) => {
                self.refuse(&session, reason, &detail).await?;
                Err(UploadError::Refused { reason, detail })
            },
            Err(error) => {
                if let Err(release) = self.uploads.release(&session.id).await {
                    tracing::warn!(upload_id = %session.id, error = %release, "Failed to release a claimed upload");
                }
                Err(error)
            },
            ok => ok,
        };
        outcome
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
            checked_kind: inspected.checked.as_str().to_owned(),
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
}
