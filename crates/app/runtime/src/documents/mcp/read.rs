//! Reads hand back hashlines so every later edit can quote a node id and
//! hash it has already seen.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::DocumentUri;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::inputs::ReadInput;
use super::lines::render;
use super::outputs::ReadOutput;
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
        "Read a document as `<node>:<hash>| <text>` lines; view outline first, then text, range \
         or cells. See documents_guide topic editing."
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
