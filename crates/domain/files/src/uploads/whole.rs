//! Uploading a body already held in memory as one session: an inline MCP
//! body, a multipart import, or outside bytes being ingested. Each runs
//! through the same quota, checks and completion as a resumable upload.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{TraceId, UserId};
use systemprompt_traits::FileStorageError;

use super::error::{UploadError, UploadResult};
use super::models::{Allowance, CompletedUpload, NewUpload, UploadSession};
use super::service::UploadService;

impl UploadService {
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
        self.put_whole(owner, request, bytes, trace, Allowance::OneShot)
            .await
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "a whole-body upload names its owner, request, bytes, trace and allowance, each distinct"
    )]
    pub async fn put_whole(
        &self,
        owner: &UserId,
        request: NewUpload,
        bytes: Vec<u8>,
        trace: &TraceId,
        allowance: Allowance,
    ) -> UploadResult<CompletedUpload> {
        let size = bytes.len() as u64;
        if size != request.size {
            return Err(UploadError::Invalid(format!(
                "declared size {} but the body is {size} bytes",
                request.size
            )));
        }
        let session = self.begin(owner, request, trace, allowance).await?;
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
