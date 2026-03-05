use crate::models::{ExecutionStatus, ToolExecutionRequest, ToolExecutionResult};
use crate::repository::{McpArtifactRepository, ToolUsageRepository};
use crate::response::McpResponseBuilder;
use crate::schema::McpOutputSchema;
use async_trait::async_trait;
use chrono::Utc;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_models::RequestContext;

#[async_trait]
pub trait McpToolHandler: Send + Sync {
    type Input: DeserializeOwned + JsonSchema + Send;
    type Output: Serialize + JsonSchema + McpOutputSchema + Send;

    fn tool_name(&self) -> &'static str;

    fn description(&self) -> &'static str {
        ""
    }

    fn input_schema(&self) -> JsonValue {
        let schema = schemars::schema_for!(Self::Input);
        serde_json::to_value(&schema).unwrap_or(JsonValue::Null)
    }

    fn output_schema(&self) -> JsonValue {
        Self::Output::validated_schema()
    }

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError>;
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
    ) -> Result<CallToolResult, McpError> {
        let started_at = Utc::now();

        let execution_request = ToolExecutionRequest {
            tool_name: handler.tool_name().to_string(),
            server_name: self.server_name.clone(),
            input: serde_json::to_value(&request.arguments).unwrap_or_default(),
            started_at,
            context: ctx.clone(),
            request_method: Some("mcp".to_string()),
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
                McpError::internal_error(
                    format!("Failed to start execution tracking: {e}"),
                    None,
                )
            })?;

        tracing::info!(tool = handler.tool_name(), %exec_id, "MCP execution started");

        let result = async {
            let input: H::Input = parse_input(request)?;
            handler.handle(input, ctx, &exec_id).await
        }
        .await;

        let response = match result {
            Ok((output, summary)) => {
                let title = output.artifact_title();
                let artifact_type = output.artifact_type_name();
                McpResponseBuilder::new(output, handler.tool_name(), ctx, &exec_id)
                    .build(summary, &self.artifact_repo, &artifact_type, title)
                    .await
            }
            Err(ref e) => Err(e.clone()),
        };

        let completed_at = Utc::now();
        let execution_result = ToolExecutionResult {
            output: response
                .as_ref()
                .ok()
                .and_then(|r| r.structured_content.clone()),
            output_schema: None,
            status: if response.is_ok() {
                ExecutionStatus::Success.as_str().to_string()
            } else {
                ExecutionStatus::Failed.as_str().to_string()
            },
            error_message: response.as_ref().err().map(|e| e.message.to_string()),
            started_at,
            completed_at,
        };

        match self
            .tool_usage_repo
            .complete_execution(&exec_id, &execution_result)
            .await
        {
            Ok(()) => {
                tracing::info!(tool = handler.tool_name(), %exec_id, "MCP execution completed");
            }
            Err(e) => {
                tracing::error!(
                    tool = handler.tool_name(),
                    %exec_id,
                    error = %e,
                    "Failed to complete execution tracking"
                );
            }
        }

        response
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
