//! Attachment bytes never pass through the tool.
//!
//! `declare` opens an upload and answers with signed links the client sends
//! the bytes to out of band, and `attach` links the completed upload. The
//! bytes go to the resumable upload API, whose content checks (including the
//! zip-bomb guard) run on completion, and the report holds the resulting
//! content-addressed blob.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_files::uploads::models::NewUpload;
use systemprompt_files::uploads::{UploadError, UploadService};
use systemprompt_identifiers::{ContentHash, DocumentUploadId, IssueReportId, McpExecutionId};
use systemprompt_mcp::{McpOutputSchema, McpToolHandler};
use systemprompt_models::execution::context::RequestContext;

use super::IssuesMcpTools;
use super::refusal::{IssueRefusalCode, caller, refusal, tool_error};
use crate::documents::upload_links::{
    UPLOAD_TOKEN_HEADER, UploadGrant, UploadLinkTarget, signed_upload_links,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentAction {
    Declare,
    Attach,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachmentInput {
    pub action: AttachmentAction,
    pub report_id: IssueReportId,
    #[schemars(description = "declare: the file name.")]
    #[serde(default)]
    pub name: Option<String>,
    #[schemars(description = "declare: the MIME type, e.g. application/zip.")]
    #[serde(default)]
    pub mime: Option<String>,
    #[schemars(description = "declare: the size in bytes.")]
    #[serde(default)]
    pub size: Option<u64>,
    #[schemars(description = "declare: the lowercase hex SHA-256 of the bytes.")]
    #[serde(default)]
    pub sha256: Option<String>,
    #[schemars(description = "attach: the completed upload's id.")]
    #[serde(default)]
    pub upload_id: Option<DocumentUploadId>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AttachmentOutput {
    pub report_id: IssueReportId,
    pub upload_id: DocumentUploadId,
    pub attached: bool,
    pub part_size: Option<u64>,
    pub parts_url: Option<String>,
    pub status_url: Option<String>,
    pub complete_url: Option<String>,
    pub upload_token: Option<String>,
    pub token_header: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl McpOutputSchema for AttachmentOutput {
    fn artifact_type() -> &'static str {
        "issue_attachment"
    }

    fn terse_wire(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AttachmentHandler<'a> {
    pub tools: &'a IssuesMcpTools,
}

fn missing(field: &str) -> McpError {
    refusal(
        IssueRefusalCode::InvalidParams,
        format!("{field} is required for this action"),
    )
}

fn upload_error(error: &UploadError) -> McpError {
    match error {
        UploadError::Storage(_)
        | UploadError::Repository(_)
        | UploadError::Blob(_)
        | UploadError::Interrupted(_)
        | UploadError::Unanswered(_) => {
            tracing::error!(error = %error, cause = ?std::error::Error::source(error), "issue attachment upload failed");
            McpError::internal_error("the upload could not be opened; try the call again", None)
        },
        _ => refusal(IssueRefusalCode::InvalidParams, error.to_string()),
    }
}

impl AttachmentHandler<'_> {
    fn uploads(&self) -> Result<&UploadService, McpError> {
        self.tools.uploads().ok_or_else(|| {
            refusal(
                IssueRefusalCode::UploadsUnavailable,
                "this install takes no attachments: files.documents is off",
            )
        })
    }

    async fn declare(
        &self,
        input: AttachmentInput,
        ctx: &RequestContext,
    ) -> Result<AttachmentOutput, McpError> {
        let user = caller(ctx)?;
        let issues = self.tools.issues();
        issues
            .ensure_attachable(&input.report_id, &user.id)
            .await
            .map_err(tool_error)?;
        let sha256 = input.sha256.ok_or_else(|| missing("sha256"))?;
        let request = NewUpload {
            name: input.name.ok_or_else(|| missing("name"))?,
            mime: input.mime.ok_or_else(|| missing("mime"))?,
            size: input.size.ok_or_else(|| missing("size"))?,
            sha256: ContentHash::try_new(sha256.as_str()).map_err(|_invalid| {
                refusal(
                    IssueRefusalCode::InvalidParams,
                    "sha256 must be 64 lowercase hex characters",
                )
            })?,
        };
        let session = self
            .uploads()?
            .create(&user.id, request, ctx.trace_id())
            .await
            .map_err(|error| upload_error(&error))?;
        let links = signed_upload_links(
            self.tools.token_authority(),
            UploadLinkTarget {
                grant: UploadGrant::Write,
                base_url: self.tools.api_external_url(),
                user,
                session_id: ctx.session_id(),
                upload: &session.id,
            },
        )
        .map_err(|error| {
            tracing::error!(error = %error, cause = ?std::error::Error::source(&error), "upload token was not minted");
            McpError::internal_error("the upload links could not be signed", None)
        })?;
        Ok(AttachmentOutput {
            report_id: input.report_id,
            upload_id: session.id,
            attached: false,
            part_size: Some(session.part_size),
            parts_url: Some(links.parts_url),
            status_url: Some(links.status_url),
            complete_url: Some(links.complete_url),
            upload_token: Some(links.token),
            token_header: Some(UPLOAD_TOKEN_HEADER.to_owned()),
            expires_at: Some(links.expires_at),
        })
    }

    async fn attach(
        &self,
        input: AttachmentInput,
        ctx: &RequestContext,
    ) -> Result<AttachmentOutput, McpError> {
        let user = caller(ctx)?;
        let upload_id = input.upload_id.ok_or_else(|| missing("upload_id"))?;
        self.tools
            .issues()
            .attach(&input.report_id, &upload_id, &user.id)
            .await
            .map_err(tool_error)?;
        Ok(AttachmentOutput {
            report_id: input.report_id,
            upload_id,
            attached: true,
            part_size: None,
            parts_url: None,
            status_url: None,
            complete_url: None,
            upload_token: None,
            token_header: None,
            expires_at: None,
        })
    }
}

impl McpToolHandler for AttachmentHandler<'_> {
    type Input = AttachmentInput;
    type Output = AttachmentOutput;

    fn tool_name(&self) -> &'static str {
        "issue_attachment"
    }

    fn description(&self) -> &'static str {
        "Attach a file to your issue report: declare (get upload links), send the bytes, attach."
    }

    async fn handle(
        &self,
        input: AttachmentInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(AttachmentOutput, String), McpError> {
        let output = match input.action {
            AttachmentAction::Declare => self.declare(input, ctx).await?,
            AttachmentAction::Attach => self.attach(input, ctx).await?,
        };
        let summary = if output.attached {
            format!(
                "attached upload {} to issue {}",
                output.upload_id, output.report_id
            )
        } else {
            format!(
                "upload {} opened for issue {}",
                output.upload_id, output.report_id
            )
        };
        Ok((output, summary))
    }
}
