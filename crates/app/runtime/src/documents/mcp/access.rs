//! `documents_access`: the owner, grants, inherited grants and the caller's
//! level on a document; for a caller holding `share`, also the open access
//! requests and, with `history: true`, the grant audit log.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::access_records::AccessOutput;
use super::inputs::AccessInput;
use super::support::{caller, tool_error};

#[derive(Debug, Clone, Copy)]
pub struct AccessHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for AccessHandler<'_> {
    type Input = AccessInput;
    type Output = AccessOutput;

    fn tool_name(&self) -> &'static str {
        "documents_access"
    }

    fn description(&self) -> &'static str {
        "Owner, grants, access requests, history and your access."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: AccessInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(AccessOutput, String), McpError> {
        let who = caller(ctx)?;
        let access = self
            .tools
            .service()
            .access_of(who, &input.document_id, input.history, ctx.trace_id())
            .await
            .map_err(tool_error)?;
        let summary = format!(
            "owner {}, {} grants, {} inherited, {} open requests; you hold {}",
            access.owner,
            access.grants.len(),
            access.inherited.len(),
            access.requests.len(),
            access.my_access
        );
        Ok((AccessOutput::from(access), summary))
    }
}
