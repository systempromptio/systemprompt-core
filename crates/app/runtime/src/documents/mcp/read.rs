//! Read-only content tools: `documents_read`, `documents_download_url` and
//! `documents_render_preview`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId, McpExecutionId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentUri;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::download::{DownloadTarget, signed_download_url};
use super::inputs::{ReadInput, RevisionInput};
use super::lines::render;
use super::outputs::{DownloadOutput, PreviewOutput, ReadOutput};
use super::preview::preview_resource_uri;
use super::refusal::{RefusalCode, refusal};
use super::support::{caller, tool_error};

#[derive(Debug, Clone, Copy)]
pub struct ReadHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for ReadHandler<'_> {
    type Input = ReadInput;
    type Output = ReadOutput;

    fn tool_name(&self) -> &'static str {
        "documents_read"
    }

    fn description(&self) -> &'static str {
        "Read a document as hashlines. view: outline (headings, slides, sheets, tables), text \
         (every node, the default), range (range.from_node to range.to_node) or cells (sheet and \
         an A1 range such as A1:C9). A line is `<node>:<hash>| <text>`, a cell \
         `<sheet>!<cell>:<hash>| <value>`; quote the node and hash in documents_edit, and \
         revision_id as base_revision."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: ReadInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(ReadOutput, String), McpError> {
        let who = caller(ctx)?;
        let (revision, projection) = self
            .tools
            .service()
            .project(
                who,
                &input.document_id,
                input.revision_id.as_ref(),
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        let rendered = render(&projection, &input, &revision.id)?;
        let summary = format!(
            "{} lines of revision {}{}",
            rendered.lines.len(),
            revision.id,
            if rendered.truncated {
                " (truncated)"
            } else {
                ""
            }
        );
        Ok((
            ReadOutput {
                uri: input
                    .verbose
                    .then(|| DocumentUri::new(input.document_id).at_revision(revision.id.clone())),
                revision_id: revision.id,
                content_sha256: input.verbose.then_some(projection.content_sha256),
                format: projection.format,
                lines: rendered.lines,
                truncated: rendered.truncated,
            },
            summary,
        ))
    }
}

async fn resolve_revision(
    tools: &DocumentsMcpTools,
    who: &AuthenticatedUser,
    input: &RevisionInput,
    ctx: &RequestContext,
) -> Result<DocumentRevisionId, McpError> {
    let service = tools.service();
    if let Some(revision) = &input.revision_id {
        return service
            .revision(who, &input.document_id, revision, ctx.trace_id())
            .await
            .map(|r| r.id)
            .map_err(tool_error);
    }
    let view = service
        .get(who, &input.document_id, ctx.trace_id())
        .await
        .map_err(tool_error)?;
    view.head
        .map(|head| head.id)
        .ok_or_else(|| no_revision(&input.document_id))
}

fn no_revision(document: &DocumentId) -> McpError {
    refusal(
        RefusalCode::NotFound,
        format!("document {document} has no revision yet; nothing to download"),
    )
}

#[derive(Debug, Clone, Copy)]
pub struct DownloadUrlHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for DownloadUrlHandler<'_> {
    type Input = RevisionInput;
    type Output = DownloadOutput;

    fn tool_name(&self) -> &'static str {
        "documents_download_url"
    }

    fn description(&self) -> &'static str {
        "Get a signed URL that downloads the document file (the head revision unless revision_id \
         is given). The URL is valid for ten minutes and acts only as you."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: RevisionInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(DownloadOutput, String), McpError> {
        let who = caller(ctx)?;
        let revision = resolve_revision(self.tools, who, &input, ctx).await?;
        let (url, expires_at) = signed_download_url(
            self.tools.token_authority(),
            DownloadTarget {
                base_url: self.tools.api_external_url(),
                user: who,
                session_id: ctx.session_id(),
                document: &input.document_id,
                revision: &revision,
            },
        )?;
        let summary = format!("download link valid until {}", expires_at.to_rfc3339());
        Ok((
            DownloadOutput {
                revision_id: revision,
                url,
                expires_at,
            },
            summary,
        ))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RenderPreviewHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for RenderPreviewHandler<'_> {
    type Input = RevisionInput;
    type Output = PreviewOutput;

    fn tool_name(&self) -> &'static str {
        "documents_render_preview"
    }

    fn description(&self) -> &'static str {
        "Render a sanitised HTML preview of a document revision (the head unless revision_id is \
         given) for display in the document-preview app."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: RevisionInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(PreviewOutput, String), McpError> {
        let who = caller(ctx)?;
        let (revision, html) = self
            .tools
            .service()
            .preview_html(
                who,
                &input.document_id,
                input.revision_id.as_ref(),
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        let summary = format!("preview of revision {}", revision.id);
        Ok((
            PreviewOutput {
                revision_id: revision.id,
                html,
                ui_resource: preview_resource_uri(self.tools.server_name()),
            },
            summary,
        ))
    }
}
