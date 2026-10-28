//! `documents_read`: the read views of [`read_view`] as a tool.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::support::{caller, tool_error};
use crate::documents::views::{ReadInput, ReadOutput, read_view};

fn summary(output: &ReadOutput) -> String {
    if let Some(meta) = &output.meta {
        return format!(
            "{} at revision {}, {} present",
            meta.name,
            output.revision_id,
            meta.editing.len()
        );
    }
    format!(
        "{} lines of revision {}{}",
        output.lines.len(),
        output.revision_id,
        if output.truncated { " (truncated)" } else { "" }
    )
}

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
        "Read `<node>:<hash>| <text>` lines; view outline first. Guide: editing."
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
        let output = read_view(self.tools.service(), who, input, ctx.trace_id())
            .await
            .map_err(tool_error)?;
        let summary = summary(&output);
        Ok((output, summary))
    }
}
