//! Per-tool audit for external MCP servers served over the HTTP gateway.
//!
//! A client-mediated `tools/call` to an external provider has no backend
//! process to record it, so the gateway taps the forwarded request/response,
//! writes one `mcp_tool_executions` row under the calling user, and hands the
//! result to the artifact ingest. The execution id is minted before the
//! response leaves, and stamped into its `_meta`, so the client's own later
//! report of the same result carries the exact server key. `record` composes
//! the tap over the upstream body; the tap owns an [`McpAudit`] and finalizes
//! it (once) on stream EOF or drop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod jsonrpc;
pub mod tap;

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde_json::Value;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_mcp::{ArtifactIngest, IngestRequest, from_wire_value};
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::{Correlation, ExecutionSource};

pub(crate) use jsonrpc::parse_tool_call;
pub(crate) use tap::record;

use jsonrpc::{ToolCallInvocation, ToolCallOutcome};

#[derive(Debug)]
pub struct McpAudit {
    repo: Arc<ToolUsageRepository>,
    ingest: Option<Arc<ArtifactIngest>>,
    context: RequestContext,
    server_name: String,
    invocation: ToolCallInvocation,
    started_at: DateTime<Utc>,
    mcp_execution_id: McpExecutionId,
}

impl McpAudit {
    pub fn new(
        repo: Arc<ToolUsageRepository>,
        ingest: Option<Arc<ArtifactIngest>>,
        context: RequestContext,
        server_name: String,
        invocation: ToolCallInvocation,
    ) -> Self {
        Self {
            repo,
            ingest,
            context,
            server_name,
            invocation,
            started_at: Utc::now(),
            mcp_execution_id: McpExecutionId::new(uuid::Uuid::new_v4().to_string()),
        }
    }

    const fn request_id(&self) -> &Value {
        &self.invocation.id
    }

    pub fn mcp_execution_id(&self) -> &McpExecutionId {
        &self.mcp_execution_id
    }

    fn finalize(self, outcome: Option<ToolCallOutcome>) {
        let (output, error_message, result) = match outcome {
            Some(o) => (o.output, o.error_message, o.result),
            None => (
                None,
                Some("external MCP tool call produced no parseable result".to_owned()),
                None,
            ),
        };

        let request = ToolExecutionRequest {
            tool_name: self.invocation.tool_name,
            server_name: self.server_name.clone(),
            input: self.invocation.arguments,
            started_at: self.started_at,
            context: self.context,
            request_method: Some("mcp".to_owned()),
            request_source: Some(self.server_name),
            ai_tool_call_id: None,
            source: ExecutionSource::Proxy,
        };
        let result_row = ToolExecutionResult {
            status: ExecutionStatus::from_error(error_message.is_some()).to_string(),
            error_message,
            output,
            output_schema: None,
            started_at: self.started_at,
            completed_at: Utc::now(),
        };

        let repo = self.repo;
        let ingest = self.ingest;
        let mcp_execution_id = self.mcp_execution_id;
        tokio::spawn(async move {
            let mut request = request;
            request.ai_tool_call_id = request.context.ai_tool_call_id().cloned();
            if let Err(e) = repo
                .log_execution_sync_with_id(
                    &mcp_execution_id,
                    &request,
                    &result_row,
                    Correlation::Exact,
                )
                .await
            {
                tracing::warn!(
                    tool = %request.tool_name,
                    server = %request.server_name,
                    error = %e,
                    "Failed to record external MCP tool execution"
                );
                return;
            }
            let Some(ingest) = ingest else {
                return;
            };
            let Some(wire) = result.as_ref().and_then(from_wire_value) else {
                return;
            };
            let ingest_request = IngestRequest {
                result: wire,
                tool_name: request.tool_name.clone(),
                server_name: Some(request.server_name.clone()),
                ai_tool_call_id: request.ai_tool_call_id.clone(),
                mcp_execution_id: Some(mcp_execution_id),
                ctx: request.context.clone(),
                skill: None,
                source: ExecutionSource::Proxy,
                started_at: Some(request.started_at),
                input: Some(request.input.clone()),
            };
            if let Err(e) = ingest.ingest(ingest_request).await {
                tracing::warn!(
                    tool = %request.tool_name,
                    server = %request.server_name,
                    error = %e,
                    "Failed to ingest external MCP tool result as an artifact"
                );
            }
        });
    }
}
