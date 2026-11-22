//! The `documents_import` tool: a document from a blob `documents_upload`
//! answered.
//!
//! The call names the `blob_ref`, the document `name` and optionally its
//! `folder_id`; the name's extension picks the format unless `format` is
//! given, and a blob of no editable format becomes a `file` document. The
//! bytes are stored as uploaded, after the format's codec has parsed them
//! whole, so an HTML page keeps its scripts and styles. Only the user who
//! uploaded a blob may import it; an editable format is imported only from
//! an upload that passed that format's content check, and a blob past the
//! document upload limit is refused, never truncated. The import is an
//! agent-authored create with the MCP execution as its origin.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::service::ImportRequest;
use systemprompt_files::uploads::BlobOrigin;
use systemprompt_identifiers::{ContentHash, McpExecutionId, UserId};
use systemprompt_extension::mcp::McpToolHandler;
use systemprompt_models::documents::DocumentFormat;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::inputs::ImportInput;
use super::outputs::CreateOutput;
use super::support::{agent_author, caller, tool_error};
use super::upload::{upload_error, uploads};
use crate::documents::views::written_lines;

pub(super) async fn owned_blob(
    tools: &DocumentsMcpTools,
    user: &UserId,
    blob: &ContentHash,
    format: DocumentFormat,
    max_bytes: u64,
) -> Result<(Vec<u8>, BlobOrigin), McpError> {
    uploads(tools)?
        .read_owned_blob(user, blob, format, max_bytes)
        .await
        .map_err(|error| upload_error(&error))
}

fn import_format(input: &ImportInput) -> DocumentFormat {
    input
        .format
        .or_else(|| {
            input
                .name
                .rsplit_once('.')
                .and_then(|(_, extension)| DocumentFormat::from_extension(extension))
        })
        .unwrap_or(DocumentFormat::File)
}

#[derive(Debug, Clone, Copy)]
pub struct ImportHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for ImportHandler<'_> {
    type Input = ImportInput;
    type Output = CreateOutput;

    fn tool_name(&self) -> &'static str {
        "documents_import"
    }

    fn description(&self) -> &'static str {
        "Import an uploaded blob_ref as a document. Guide: uploads."
    }

    async fn handle(
        &self,
        input: ImportInput,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> Result<(CreateOutput, String), McpError> {
        let who = caller(ctx)?;
        let service = self.tools.service();
        let format = import_format(&input);
        let (bytes, origin) = owned_blob(
            self.tools,
            ctx.user_id(),
            &input.blob_ref,
            format,
            service.config().max_upload_bytes,
        )
        .await?;
        let (document, revision) = service
            .import(
                who,
                ImportRequest {
                    folder: input.folder_id,
                    name: input.name,
                    format: Some(format),
                    mime_type: Some(origin.mime),
                    bytes,
                    author: agent_author(self.tools, ctx, exec_id, input.context_id.as_ref())
                        .await?,
                    message: input.message,
                },
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        let (_, projection) = service
            .project(who, &document.id, Some(&revision.id), ctx.trace_id())
            .await
            .map_err(tool_error)?;
        let written = written_lines(None, &projection);
        let summary = format!(
            "imported {} as {} at revision {}: {} lines written",
            document.name,
            document.format,
            revision.id,
            written.len()
        );
        Ok((
            CreateOutput {
                document_id: document.id,
                revision_id: revision.id,
                format: document.format,
                name: document.name,
                written,
            },
            summary,
        ))
    }
}
