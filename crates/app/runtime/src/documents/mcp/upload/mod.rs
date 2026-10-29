//! The `documents_upload` tool puts a file into the store as a blob an
//! import can name.
//!
//! A body up to the inline limit (`files.documents.uploads.inlineMaxBytes`,
//! 5 MiB by default) is sent as base64 in `content_base64` and answered
//! with its `blob_ref`. A larger file is declared by `size` and `sha256`
//! and answered with a resumable session: URLs to `PUT` each part
//! (`parts_url`, `{n}` replaced by the 1-based part number, every part
//! `part_size` bytes but the last), read progress and complete it, and an
//! `upload_token` valid for fifteen minutes and for that upload only, sent in
//! the `token_header` header; the completion answers the `blob_ref`. Calling
//! again with just `upload_id` re-issues the token for an unfinished upload.
//! Either way the content passes the same checks as a browser upload.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use rmcp::ErrorData as McpError;
use systemprompt_files::uploads::{
    NewUpload, UploadError, UploadService, UploadSession, UploadStatus,
};
use systemprompt_identifiers::{ContentHash, DocumentUploadId, McpExecutionId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::content_hash;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::refusal::refusal;
use super::support::caller;
use crate::documents::refusal::RefusalCode;
use crate::documents::upload_links::{
    UPLOAD_TOKEN_HEADER, UploadGrant, UploadLinkTarget, signed_upload_links,
};

mod io;

pub use io::{UploadInput, UploadMode, UploadOutput};

pub(super) fn upload_error(error: &UploadError) -> McpError {
    let code = match error {
        UploadError::TooLarge { .. }
        | UploadError::QuotaExceeded { .. }
        | UploadError::PartBudgetExhausted { .. } => RefusalCode::TooLarge,
        UploadError::NotFound | UploadError::BlobNotOwned(_) => RefusalCode::NotFound,
        UploadError::Refused { .. } => RefusalCode::IntegrityMismatch,
        UploadError::Storage(_)
        | UploadError::Repository(_)
        | UploadError::Blob(_)
        | UploadError::Interrupted(_)
        | UploadError::Unanswered(_) => {
            tracing::error!(error = %error, cause = ?std::error::Error::source(error), "document upload failed");
            return McpError::internal_error("the upload could not be stored", None);
        },
        _ => RefusalCode::InvalidParams,
    };
    refusal(code, error.to_string())
}

fn uploads(tools: &DocumentsMcpTools) -> Result<&UploadService, McpError> {
    tools.uploads().ok_or_else(|| {
        refusal(
            RefusalCode::InvalidParams,
            "this server does not accept uploads",
        )
    })
}

#[derive(Debug, Clone, Copy)]
pub struct UploadHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl UploadHandler<'_> {
    async fn inline(
        &self,
        input: UploadInput,
        encoded: &str,
        ctx: &RequestContext,
    ) -> Result<(UploadOutput, String), McpError> {
        let service = uploads(self.tools)?;
        let limit = service.limits().inline_max_bytes;
        if (encoded.len() as u64).saturating_mul(3) / 4 > limit {
            return Err(refusal(
                RefusalCode::TooLarge,
                format!(
                    "inline bodies are limited to {limit} bytes; declare size and sha256 for a resumable upload"
                ),
            ));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .map_err(|_undecodable| {
                refusal(
                    RefusalCode::InvalidParams,
                    "content_base64 is not valid base64".to_owned(),
                )
            })?;
        let request = NewUpload {
            name: input.name,
            mime: input.mime,
            size: bytes.len() as u64,
            sha256: content_hash(&bytes),
        };
        let done = service
            .put_inline(ctx.user_id(), request, bytes, ctx.trace_id())
            .await
            .map_err(|error| upload_error(&error))?;
        let summary = format!("uploaded as blob {} ({} bytes)", done.blob, done.size_bytes);
        Ok((
            UploadOutput {
                mode: UploadMode::Inline,
                upload_id: done.session.id,
                blob_ref: Some(done.blob),
                mime: Some(done.detected_mime),
                deduplicated: Some(done.deduplicated),
                part_size: None,
                part_count: None,
                parts_url: None,
                status_url: None,
                complete_url: None,
                upload_token: None,
                token_header: None,
                expires_at: None,
            },
            summary,
        ))
    }

    async fn resumable(
        &self,
        input: UploadInput,
        size: u64,
        sha256: ContentHash,
        ctx: &RequestContext,
    ) -> Result<(UploadOutput, String), McpError> {
        let request = NewUpload {
            name: input.name,
            mime: input.mime,
            size,
            sha256,
        };
        let session = uploads(self.tools)?
            .create(ctx.user_id(), request, ctx.trace_id())
            .await
            .map_err(|error| upload_error(&error))?;
        self.signed(&session, ctx)
    }

    async fn reissue(
        &self,
        upload_id: &DocumentUploadId,
        ctx: &RequestContext,
    ) -> Result<(UploadOutput, String), McpError> {
        let progress = uploads(self.tools)?
            .progress(ctx.user_id(), upload_id)
            .await
            .map_err(|error| upload_error(&error))?;
        if progress.session.status != UploadStatus::Open {
            return Err(upload_error(&UploadError::Closed {
                status: progress.session.status,
            }));
        }
        self.signed(&progress.session, ctx)
    }

    fn signed(
        &self,
        session: &UploadSession,
        ctx: &RequestContext,
    ) -> Result<(UploadOutput, String), McpError> {
        let links = signed_upload_links(
            self.tools.token_authority(),
            UploadLinkTarget {
                grant: UploadGrant::Write,
                base_url: self.tools.api_external_url(),
                user: caller(ctx)?,
                session_id: ctx.session_id(),
                upload: &session.id,
            },
        )
        .map_err(|error| {
            tracing::error!(error = %error, cause = ?std::error::Error::source(&error), "upload token was not minted");
            McpError::internal_error("the upload links could not be signed", None)
        })?;
        let summary = format!(
            "PUT {} parts of {} bytes to parts_url, then POST complete_url, sending upload_token \
             in the {UPLOAD_TOKEN_HEADER} header; when it expires call documents_upload with \
             upload_id for a new one",
            session.part_count(),
            session.part_size
        );
        Ok((
            UploadOutput {
                mode: UploadMode::Resumable,
                part_count: Some(session.part_count()),
                part_size: Some(session.part_size),
                upload_id: session.id.clone(),
                blob_ref: None,
                mime: None,
                deduplicated: None,
                parts_url: Some(links.parts_url),
                status_url: Some(links.status_url),
                complete_url: Some(links.complete_url),
                upload_token: Some(links.token),
                token_header: Some(UPLOAD_TOKEN_HEADER.to_owned()),
                expires_at: Some(links.expires_at.min(session.expires_at)),
            },
            summary,
        ))
    }
}

impl McpToolHandler for UploadHandler<'_> {
    type Input = UploadInput;
    type Output = UploadOutput;

    fn tool_name(&self) -> &'static str {
        "documents_upload"
    }

    fn description(&self) -> &'static str {
        "Upload a file; returns a blob_ref. Guide: uploads."
    }

    async fn handle(
        &self,
        mut input: UploadInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(UploadOutput, String), McpError> {
        caller(ctx)?;
        if let Some(upload_id) = input.upload_id.clone() {
            return self.reissue(&upload_id, ctx).await;
        }
        match (
            input.content_base64.take(),
            input.size,
            input.sha256.clone(),
        ) {
            (Some(encoded), _, _) => self.inline(input, &encoded, ctx).await,
            (None, Some(size), Some(sha256)) => self.resumable(input, size, sha256, ctx).await,
            (None, _, _) => Err(refusal(
                RefusalCode::InvalidParams,
                "pass content_base64, or size and sha256 for a resumable upload",
            )),
        }
    }
}
