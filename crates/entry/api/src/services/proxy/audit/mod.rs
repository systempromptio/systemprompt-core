//! Per-tool audit for external MCP servers served over the HTTP gateway.
//!
//! A client-mediated `tools/call` to an external provider has no backend
//! process to record it, so the gateway taps the forwarded request/response and
//! writes one `mcp_tool_executions` row under the calling user. `record`
//! composes the tap over the upstream body; the tap owns an [`McpAudit`] and
//! finalizes it (once) on stream EOF or drop.

mod jsonrpc;
mod tap;

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde_json::Value;
use systemprompt_mcp::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_models::RequestContext;

pub(crate) use jsonrpc::parse_tool_call;
pub(crate) use tap::record;

use jsonrpc::{ToolCallInvocation, ToolCallOutcome};

#[cfg(feature = "test-api")]
pub mod test_api {
    use serde_json::Value;

    #[must_use]
    pub fn parse_tool_call(body: &[u8]) -> Option<(Value, String, Value)> {
        super::jsonrpc::parse_tool_call(body).map(|i| (i.id, i.tool_name, i.arguments))
    }

    #[must_use]
    pub fn parse_response_frame(
        data: &str,
        request_id: &Value,
    ) -> Option<(Option<Value>, Option<String>)> {
        super::jsonrpc::parse_response_frame(data, request_id).map(|o| (o.output, o.error_message))
    }

    #[must_use]
    pub fn extract_sse_data(frame: &str) -> Option<String> {
        super::jsonrpc::extract_sse_data(frame)
    }

    pub async fn record_tool_call(
        response: reqwest::Response,
        pool: &systemprompt_database::DbPool,
        context: systemprompt_models::RequestContext,
        server_name: &str,
        request_body: &[u8],
    ) -> Result<axum::response::Response<axum::body::Body>, String> {
        let invocation = super::jsonrpc::parse_tool_call(request_body)
            .ok_or_else(|| "request body is not a tools/call".to_owned())?;
        let repo = systemprompt_mcp::repository::ToolUsageRepository::new(pool)
            .map_err(|e| e.to_string())?;
        let audit = super::McpAudit::new(
            std::sync::Arc::new(repo),
            context,
            server_name.to_owned(),
            invocation,
        );
        super::tap::record(response, audit).await
    }
}

pub(crate) struct McpAudit {
    repo: Arc<ToolUsageRepository>,
    context: RequestContext,
    server_name: String,
    invocation: ToolCallInvocation,
    started_at: DateTime<Utc>,
}

impl McpAudit {
    pub(crate) fn new(
        repo: Arc<ToolUsageRepository>,
        context: RequestContext,
        server_name: String,
        invocation: ToolCallInvocation,
    ) -> Self {
        Self {
            repo,
            context,
            server_name,
            invocation,
            started_at: Utc::now(),
        }
    }

    const fn request_id(&self) -> &Value {
        &self.invocation.id
    }

    fn finalize(self, outcome: Option<ToolCallOutcome>) {
        let (output, error_message) = match outcome {
            Some(o) => (o.output, o.error_message),
            None => (
                None,
                Some("external MCP tool call produced no parseable result".to_owned()),
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
        };
        let result = ToolExecutionResult {
            status: ExecutionStatus::from_error(error_message.is_some()).to_string(),
            error_message,
            output,
            output_schema: None,
            started_at: self.started_at,
            completed_at: Utc::now(),
        };

        let repo = self.repo;
        tokio::spawn(async move {
            if let Err(e) = repo.log_execution_sync(&request, &result).await {
                tracing::warn!(
                    tool = %request.tool_name,
                    server = %request.server_name,
                    error = %e,
                    "Failed to record external MCP tool execution"
                );
            }
        });
    }
}
