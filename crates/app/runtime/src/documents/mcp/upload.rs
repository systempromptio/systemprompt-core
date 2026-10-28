//! `documents_upload`: a file into the store as a blob an import can name.
//!
//! A body up to the inline limit (`files.documents.uploads.inlineMaxBytes`,
//! 5 MiB by default) is sent as base64 in `content_base64` and answered
//! with its `blob_ref`. A larger file is declared by `size` and `sha256`
//! and answered with a resumable session: signed URLs, valid for an hour,
//! to `PUT` each part (`parts_url`, `{n}` replaced by the 1-based part
//! number, every part `part_size` bytes but the last), read progress and
//! complete it; the completion answers the `blob_ref`. Either way the
//! content passes the same checks as a browser upload.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use chrono::{DateTime, Utc};
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_files::uploads::{NewUpload, UploadError, UploadService};
use systemprompt_identifiers::{ContentHash, DocumentUploadId, McpExecutionId};
use systemprompt_mcp::{McpOutputSchema, McpToolHandler};
use systemprompt_models::documents::content_hash;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::outputs::documents_output;
use super::refusal::refusal;
use super::support::caller;
use crate::documents::refusal::RefusalCode;
use crate::documents::upload_links::{UploadGrant, UploadLinkTarget, signed_upload_links};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct UploadInput {
    pub name: String,
    pub mime: String,
    #[serde(default)]
    pub content_base64: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub sha256: Option<ContentHash>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UploadMode {
    Inline,
    Resumable,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UploadOutput {
    pub mode: UploadMode,
    pub upload_id: DocumentUploadId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob_ref: Option<ContentHash>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deduplicated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parts_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complete_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
}
documents_output!(UploadOutput, "documents_upload");

pub(super) fn upload_error(error: UploadError) -> McpError {
    let code = match &error {
        UploadError::TooLarge { .. }
        | UploadError::QuotaExceeded { .. }
        | UploadError::PartBudgetExhausted { .. } => RefusalCode::TooLarge,
        UploadError::NotFound | UploadError::BlobNotOwned(_) => RefusalCode::NotFound,
        UploadError::Refused { .. } => RefusalCode::IntegrityMismatch,
        UploadError::Storage(_) | UploadError::Repository(_) | UploadError::Blob(_) => {
            tracing::error!(error = %error, cause = ?std::error::Error::source(&error), "document upload failed");
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
            .map_err(|error| {
                refusal(
                    RefusalCode::InvalidParams,
                    format!("content_base64 is not base64: {error}"),
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
            .map_err(upload_error)?;
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
        let who = caller(ctx)?;
        let service = uploads(self.tools)?;
        let request = NewUpload {
            name: input.name,
            mime: input.mime,
            size,
            sha256,
        };
        let session = service
            .create(ctx.user_id(), request, ctx.trace_id())
            .await
            .map_err(upload_error)?;
        let links = signed_upload_links(
            self.tools.token_authority(),
            UploadLinkTarget {
                grant: UploadGrant::Write,
                base_url: self.tools.api_external_url(),
                user: who,
                session_id: ctx.session_id(),
                upload: &session.id,
            },
        )
        .map_err(|error| {
            tracing::error!(error = %error, cause = ?std::error::Error::source(&error), "upload token was not minted");
            McpError::internal_error("the upload links could not be signed", None)
        })?;
        let summary = format!(
            "PUT {} parts of {} bytes to parts_url, then POST complete_url",
            session.part_count(),
            session.part_size
        );
        Ok((
            UploadOutput {
                mode: UploadMode::Resumable,
                part_count: Some(session.part_count()),
                part_size: Some(session.part_size),
                upload_id: session.id,
                blob_ref: None,
                mime: None,
                deduplicated: None,
                parts_url: Some(links.parts_url),
                status_url: Some(links.status_url),
                complete_url: Some(links.complete_url),
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
        "Upload a file: base64 up to 5 MiB returns blob_ref; else size+sha256 returns signed part URLs."
    }

    async fn handle(
        &self,
        mut input: UploadInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(UploadOutput, String), McpError> {
        caller(ctx)?;
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
