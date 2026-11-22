//! The tool-execution seam: what a handler's run is recorded and shaped
//! through, without the toolset naming the MCP domain.
//!
//! [`execute`] runs one typed [`McpToolHandler`] against a
//! [`ToolExecution`]: [`ToolExecution::begin`] records the start of the
//! execution under the request context it is handed and returns the context
//! the handler runs with (an AI tool-call intent claimed when one matches);
//! the handler runs; [`ToolExecution::finish`] persists the output as an
//! artifact, shapes the result for the negotiated client and records the
//! completion. The MCP member provides the implementation per server through
//! [`ToolExecutions`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;
use systemprompt_identifiers::{McpExecutionId, McpServerId, McpToolName};
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::ClientProfile;

use super::handler::McpToolHandler;
use super::schema::McpOutputSchema;

/// One execution in flight: its id, the tool, the context the handler runs
/// with and when it started.
#[derive(Debug, Clone)]
pub struct ToolRun {
    pub exec_id: McpExecutionId,
    pub tool_name: McpToolName,
    pub context: RequestContext,
    pub started_at: DateTime<Utc>,
}

/// A handler's output, erased from its type: the serialized value and what the
/// response shaping reads of its artifact contract.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    // JSON: the tool's typed output, serialized; its schema is the tool's own.
    pub value: JsonValue,
    pub summary: String,
    pub artifact_type: String,
    pub title: Option<String>,
    pub text_body: Option<String>,
    pub terse_wire: bool,
}

/// Records and shapes one tool execution for one MCP server.
///
/// The executor records usage and audit as the request context it is handed:
/// that context is the authenticated caller the host resolved, so the seam is
/// open (rule 111): it reads no secret and acts for no subject of its own.
/// Injected as `dyn ToolExecution`, hence `#[async_trait]`.
#[async_trait]
pub trait ToolExecution: Send + Sync + Debug {
    async fn begin(
        &self,
        tool_name: McpToolName,
        request: &CallToolRequestParams,
        context: &RequestContext,
    ) -> Result<ToolRun, McpError>;

    async fn finish(
        &self,
        run: ToolRun,
        outcome: Result<ToolOutput, McpError>,
        client: &ClientProfile,
    ) -> Result<CallToolResult, McpError>;
}

pub type DynToolExecution = Arc<dyn ToolExecution>;

/// The per-server tool executions the MCP member provides.
pub trait ToolExecutions: Send + Sync + Debug {
    fn for_server(&self, server: McpServerId) -> DynToolExecution;
}

pub type DynToolExecutions = Arc<dyn ToolExecutions>;

pub async fn execute<H: McpToolHandler>(
    execution: &dyn ToolExecution,
    handler: &H,
    request: &CallToolRequestParams,
    context: &RequestContext,
    client: &ClientProfile,
) -> Result<CallToolResult, McpError> {
    let run = execution
        .begin(McpToolName::new(handler.tool_name()), request, context)
        .await?;
    let result = async {
        let input: H::Input = parse_input(request)?;
        handler.handle(input, &run.context, &run.exec_id).await
    }
    .await;
    let outcome = result.and_then(|(output, summary)| erase(handler.tool_name(), &output, summary));
    execution.finish(run, outcome, client).await
}

fn erase<O: serde::Serialize + McpOutputSchema>(
    tool: &str,
    output: &O,
    summary: String,
) -> Result<ToolOutput, McpError> {
    let value = serde_json::to_value(output).map_err(|e| {
        tracing::error!(error = %e, tool, "Failed to serialize tool output");
        McpError::internal_error("Serialization error", None)
    })?;
    Ok(ToolOutput {
        value,
        summary,
        artifact_type: output.artifact_type_name(),
        title: output.artifact_title(),
        text_body: output.text_body(),
        terse_wire: output.terse_wire(),
    })
}

fn parse_input<T: DeserializeOwned>(request: &CallToolRequestParams) -> Result<T, McpError> {
    let args_value = request
        .arguments
        .as_ref()
        .map_or(JsonValue::Object(serde_json::Map::new()), |m| {
            JsonValue::Object(m.clone())
        });

    serde_json::from_value(args_value).map_err(|e| {
        tracing::warn!(
            error = %e,
            tool = %request.name,
            "Failed to parse tool input"
        );
        McpError::invalid_params(format!("Invalid tool input: {e}"), None)
    })
}
