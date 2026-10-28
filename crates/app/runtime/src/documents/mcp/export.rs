//! A link opens the stored file, so a link in another format is offered only
//! when the document already is that format. No codec renders PDF, so a PDF
//! export is refused as `unsupported_format`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::{DocumentRevisionId, McpExecutionId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentFormat;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::inputs::{ExportAs, ExportFormat, ExportInput};
use super::outputs::ExportOutput;
use super::preview::preview_resource_uri;
use super::refusal::refusal;
use super::support::{caller, tool_error};
use crate::documents::download::{DownloadTarget, signed_download_url};
use crate::documents::refusal::RefusalCode;

enum Plan {
    Link,
    Preview,
}

fn plan(input: &ExportInput, format: DocumentFormat) -> Result<Plan, McpError> {
    match (input.format, input.export_as) {
        (ExportFormat::Native | ExportFormat::Html, ExportAs::Preview) => Ok(Plan::Preview),
        (ExportFormat::Native, ExportAs::Url) => Ok(Plan::Link),
        (ExportFormat::Md, ExportAs::Url) if format == DocumentFormat::Markdown => Ok(Plan::Link),
        (ExportFormat::Html, ExportAs::Url) if format == DocumentFormat::Html => Ok(Plan::Link),
        (ExportFormat::Pdf, _) => Err(refusal(
            RefusalCode::UnsupportedFormat,
            format!(
                "no codec renders {format} documents as PDF; export native as url, or html as \
                 preview"
            ),
        )),
        (_, _) => Err(refusal(
            RefusalCode::UnsupportedFormat,
            format!(
                "a {format} document exports as url only in its own format; export native as \
                 url, or html as preview"
            ),
        )),
    }
}

async fn revision(
    tools: &DocumentsMcpTools,
    who: &AuthenticatedUser,
    input: &ExportInput,
    ctx: &RequestContext,
) -> Result<(DocumentFormat, DocumentRevisionId), McpError> {
    let service = tools.service();
    let view = service
        .get(who, &input.document_id, ctx.trace_id())
        .await
        .map_err(tool_error)?;
    let format = view.document.format;
    if let Some(revision) = &input.revision_id {
        let found = service
            .revision(who, &input.document_id, revision, ctx.trace_id())
            .await
            .map_err(tool_error)?;
        return Ok((format, found.id));
    }
    let head = view.head.ok_or_else(|| {
        refusal(
            RefusalCode::NotFound,
            format!(
                "document {} has no revision yet; nothing to export",
                input.document_id
            ),
        )
    })?;
    Ok((format, head.id))
}

#[derive(Debug, Clone, Copy)]
pub struct ExportHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for ExportHandler<'_> {
    type Input = ExportInput;
    type Output = ExportOutput;

    fn tool_name(&self) -> &'static str {
        "documents_export"
    }

    fn description(&self) -> &'static str {
        "Export as a signed download url or an HTML preview."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: ExportInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(ExportOutput, String), McpError> {
        let who = caller(ctx)?;
        let (format, revision) = revision(self.tools, who, &input, ctx).await?;
        match plan(&input, format)? {
            Plan::Link => {
                let (url, expires_at) = signed_download_url(
                    self.tools.token_authority(),
                    DownloadTarget {
                        base_url: self.tools.api_external_url(),
                        user: who,
                        session_id: ctx.session_id(),
                        document: &input.document_id,
                        revision: &revision,
                    },
                )
                .map_err(|error| {
                    tracing::error!(error = %error, cause = ?std::error::Error::source(&error), "document download token was not minted");
                    McpError::internal_error("the download link could not be signed", None)
                })?;
                let summary = format!("download link valid until {}", expires_at.to_rfc3339());
                Ok((
                    ExportOutput {
                        revision_id: revision,
                        format,
                        url: Some(url),
                        expires_at: Some(expires_at),
                        html: None,
                        ui_resource: None,
                    },
                    summary,
                ))
            },
            Plan::Preview => {
                let (revision, html) = self
                    .tools
                    .service()
                    .preview_html(who, &input.document_id, Some(&revision), ctx.trace_id())
                    .await
                    .map_err(tool_error)?;
                let summary = format!("preview of revision {}", revision.id);
                Ok((
                    ExportOutput {
                        revision_id: revision.id,
                        format,
                        url: None,
                        expires_at: None,
                        html: Some(html),
                        ui_resource: Some(preview_resource_uri(self.tools.server_name())),
                    },
                    summary,
                ))
            },
        }
    }
}
