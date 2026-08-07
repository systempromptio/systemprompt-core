//! Typed MCP tool definition and execution wrapper.
//!
//! [`McpToolHandler`] is the contract a tool implements — typed input and
//! output with derived JSON schemas plus an async `handle` — and
//! [`McpToolExecutor`] runs a handler against a [`CallToolRequestParams`],
//! recording execution start/completion in the tool-usage repository and
//! building the [`CallToolResult`] (including any artifact) from the output.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
use crate::repository::{McpArtifactRepository, ToolUsageRepository};
use crate::response::{McpResponseBuilder, ToolIdentity};
use crate::schema::McpOutputSchema;
use chrono::Utc;
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::ClientProfile;

pub trait McpToolHandler: Send + Sync {
    type Input: DeserializeOwned + JsonSchema + Send;
    type Output: Serialize + JsonSchema + McpOutputSchema + Send;

    fn tool_name(&self) -> &'static str;

    fn description(&self) -> &'static str {
        ""
    }

    fn input_schema(&self) -> JsonValue {
        let schema = schemars::schema_for!(Self::Input);
        match serde_json::to_value(&schema) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "Failed to serialize input schema");
                JsonValue::Null
            },
        }
    }

    fn output_schema(&self) -> JsonValue {
        Self::Output::validated_schema()
    }

    /// Whether this tool only reads state. Advertised as the MCP
    /// `readOnlyHint` annotation, which lets a host serve cached results when
    /// the transport stalls instead of rendering an empty panel.
    fn read_only(&self) -> bool {
        false
    }

    /// The canonical `tools/list` entry for this handler: name, description,
    /// both schemas, the `readOnlyHint` annotation, and the UI meta. Servers
    /// hand-rolling `Tool` values drift from the wire contract; build them
    /// here.
    fn tool_definition(&self, server_name: &str) -> rmcp::model::Tool {
        let input_obj = self.input_schema().as_object().cloned().unwrap_or_default();
        let output_obj = self
            .output_schema()
            .as_object()
            .cloned()
            .unwrap_or_default();

        let mut tool = rmcp::model::Tool::default();
        tool.name = self.tool_name().to_owned().into();
        tool.description = Some(self.description().to_owned().into());
        tool.input_schema = Arc::new(input_obj);
        tool.output_schema = Some(Arc::new(output_obj));
        tool.annotations = self
            .read_only()
            .then(|| rmcp::model::ToolAnnotations::new().read_only(true));
        tool.meta = Some(rmcp::model::MetaObject(crate::capabilities::tool_ui_meta(
            server_name,
            &crate::capabilities::default_tool_visibility(),
        )));
        tool
    }

    fn handle(
        &self,
        input: Self::Input,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send;
}

#[derive(Clone, Debug)]
pub struct McpToolExecutor {
    tool_usage_repo: Arc<ToolUsageRepository>,
    artifact_repo: Arc<McpArtifactRepository>,
    server_name: String,
}

impl McpToolExecutor {
    pub fn new(
        tool_usage_repo: Arc<ToolUsageRepository>,
        artifact_repo: Arc<McpArtifactRepository>,
        server_name: impl Into<String>,
    ) -> Self {
        Self {
            tool_usage_repo,
            artifact_repo,
            server_name: server_name.into(),
        }
    }

    pub async fn execute<H: McpToolHandler>(
        &self,
        handler: &H,
        request: &CallToolRequestParams,
        ctx: &RequestContext,
        client: &ClientProfile,
    ) -> Result<CallToolResult, McpError> {
        let started_at = Utc::now();

        let input_value = serde_json::to_value(&request.arguments).map_err(|e| {
            tracing::error!(error = %e, "Failed to serialize tool arguments");
            McpError::internal_error(format!("Failed to serialize arguments: {e}"), None)
        })?;

        let execution_request = ToolExecutionRequest {
            tool_name: handler.tool_name().to_owned(),
            server_name: self.server_name.clone(),
            input: input_value,
            started_at,
            context: ctx.clone(),
            request_method: Some("mcp".to_owned()),
            request_source: Some(self.server_name.clone()),
            ai_tool_call_id: None,
        };

        let exec_id = self
            .tool_usage_repo
            .start_execution(&execution_request)
            .await
            .map_err(|e| {
                tracing::error!(
                    tool = handler.tool_name(),
                    error = %e,
                    "Failed to start execution tracking"
                );
                McpError::internal_error(format!("Failed to start execution tracking: {e}"), None)
            })?;

        tracing::info!(tool = handler.tool_name(), %exec_id, "MCP execution started");

        let result = async {
            let input: H::Input = parse_input(request)?;
            handler.handle(input, ctx, &exec_id).await
        }
        .await;

        let (response, output_value) = match result {
            Ok((output, summary)) => {
                let title = output.artifact_title();
                let artifact_type = output.artifact_type_name();
                let output_value = serde_json::to_value(&output).ok();
                let identity = ToolIdentity::new(&self.server_name, handler.tool_name());
                let response = McpResponseBuilder::new(output, identity, ctx, &exec_id, client)
                    .build(summary, &self.artifact_repo, &artifact_type, title)
                    .await;
                (response, output_value)
            },
            Err(ref e) => (Err(e.clone()), None),
        };

        let execution_result = Self::build_execution_result(&response, output_value, started_at);
        self.record_completion(handler.tool_name(), &exec_id, &execution_result)
            .await;

        response
    }

    fn build_execution_result(
        response: &Result<CallToolResult, McpError>,
        output_value: Option<JsonValue>,
        started_at: chrono::DateTime<Utc>,
    ) -> ToolExecutionResult {
        let completed_at = Utc::now();
        ToolExecutionResult {
            output: response.as_ref().ok().and(output_value),
            output_schema: None,
            status: if response.is_ok() {
                ExecutionStatus::Success.as_str().to_owned()
            } else {
                ExecutionStatus::Failed.as_str().to_owned()
            },
            error_message: response.as_ref().err().map(|e| e.message.to_string()),
            started_at,
            completed_at,
        }
    }

    async fn record_completion(
        &self,
        tool_name: &str,
        exec_id: &McpExecutionId,
        result: &ToolExecutionResult,
    ) {
        match self
            .tool_usage_repo
            .complete_execution(exec_id, result)
            .await
        {
            Ok(()) => {
                tracing::info!(tool = tool_name, %exec_id, "MCP execution completed");
            },
            Err(e) => {
                tracing::error!(
                    tool = tool_name,
                    %exec_id,
                    error = %e,
                    "Failed to complete execution tracking"
                );
            },
        }
    }
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
