//! A link opens the stored file, so a link in another format is offered only
//! when the document already is that format, except PDF: a `pdf` url is a
//! link to the revision rendered by the core PDF renderer, for every format
//! but `file`, whose only link is its own bytes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::{DocumentRevisionId, JwtToken, McpExecutionId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentFormat;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::inputs::{ExportAs, ExportFormat, ExportInput};
use super::outputs::ExportOutput;
use super::refusal::refusal;
use super::support::{caller, tool_error};
use crate::documents::download::Rendition;
use crate::documents::links::{LinkTarget, issue_download_link};
use crate::documents::refusal::RefusalCode;

enum Plan {
    Link(Rendition),
    Preview,
}

fn plan(input: &ExportInput, format: DocumentFormat) -> Result<Plan, McpError> {
    if format.is_file() && !matches!(input.format, ExportFormat::Native | ExportFormat::Html) {
        return Err(refusal(
            RefusalCode::UnsupportedFormat,
            "a file document exports as native url (its own bytes) or html preview only".to_owned(),
        ));
    }
    match (input.format, input.export_as) {
        (ExportFormat::Native | ExportFormat::Html, ExportAs::Preview) => Ok(Plan::Preview),
        (ExportFormat::Native, ExportAs::Url) => Ok(Plan::Link(Rendition::Native)),
        (ExportFormat::Pdf, ExportAs::Url) => Ok(Plan::Link(Rendition::Pdf)),
        (ExportFormat::Md, ExportAs::Url) if format == DocumentFormat::Markdown => {
            Ok(Plan::Link(Rendition::Native))
        },
        (ExportFormat::Html, ExportAs::Url) if format == DocumentFormat::Html => {
            Ok(Plan::Link(Rendition::Native))
        },
        (ExportFormat::Pdf, ExportAs::Preview) => Err(refusal(
            RefusalCode::UnsupportedFormat,
            "pdf exports as url only; export pdf as url, or html as preview".to_owned(),
        )),
        (_, _) => Err(refusal(
            RefusalCode::UnsupportedFormat,
            format!(
                "a {format} document exports as url only in its own format or as pdf; export \
                 native or pdf as url, or html as preview"
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
    pub hop: Option<&'a JwtToken>,
}

impl McpToolHandler for ExportHandler<'_> {
    type Input = ExportInput;
    type Output = ExportOutput;

    fn tool_name(&self) -> &'static str {
        "documents_export"
    }

    fn description(&self) -> &'static str {
        "Export a url or HTML preview."
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
            Plan::Link(rendition) => {
                let (url, expires_at) = issue_download_link(
                    self.tools.link_issuer(),
                    self.hop,
                    LinkTarget::Download {
                        document_id: input.document_id.clone(),
                        revision_id: revision.clone(),
                        rendition,
                    },
                )
                .await
                .map_err(|error| {
                    tracing::error!(error = %error, cause = ?std::error::Error::source(&error), "document download link was not issued by the API");
                    McpError::internal_error(error.tool_message("download link"), None)
                })?;
                let summary = format!("download link valid until {}", expires_at.to_rfc3339());
                Ok((
                    ExportOutput {
                        revision_id: revision,
                        format,
                        url: Some(url),
                        expires_at: Some(expires_at),
                        html: None,
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
                    },
                    summary,
                ))
            },
        }
    }
}
