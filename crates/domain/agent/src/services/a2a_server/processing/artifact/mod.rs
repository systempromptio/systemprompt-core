//! Construction of A2A [`Artifact`]s from MCP tool results.
//!
//! [`ArtifactBuilder`] pairs each tool call with its structured result and
//! transforms it into an A2A artifact via [`McpToA2aTransformer`], skipping
//! results without structured content.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::{AgentServiceError, Result};
use systemprompt_identifiers::{ContextId, McpToolName, TaskId};
use systemprompt_models::{CallToolResult, McpTool, ToolCall};

use crate::models::a2a::Artifact;
use crate::services::mcp::McpToA2aTransformer;

#[derive(Debug)]
pub struct ArtifactBuilder {
    tool_calls: Vec<ToolCall>,
    tool_results: Vec<CallToolResult>,
    tools: Vec<McpTool>,
    context_id: ContextId,
    task_id: TaskId,
}

impl ArtifactBuilder {
    pub const fn new(
        tool_calls: Vec<ToolCall>,
        tool_results: Vec<CallToolResult>,
        tools: Vec<McpTool>,
        context_id: ContextId,
        task_id: TaskId,
    ) -> Self {
        Self {
            tool_calls,
            tool_results,
            tools,
            context_id,
            task_id,
        }
    }

    // JSON: MCP tool output schema — arbitrary JSON Schema.
    fn get_output_schema(&self, tool_name: &McpToolName) -> Option<&serde_json::Value> {
        self.tools
            .iter()
            .find(|t| t.name == tool_name.as_str())
            .and_then(|t| t.output_schema.as_ref())
    }

    pub fn build_artifacts(&self) -> Result<Vec<Artifact>> {
        let mut artifacts = Vec::new();

        for (index, result) in self.tool_results.iter().enumerate() {
            if result
                .structured_content
                .as_ref()
                .is_some_and(|v| !v.is_null())
                && let Some(tool_call) = self.tool_calls.get(index)
            {
                let tool_name = McpToolName::try_new(tool_call.name.as_str()).map_err(|e| {
                    AgentServiceError::operation(
                        format!(
                            "Tool call {} has no usable tool name",
                            tool_call.ai_tool_call_id
                        ),
                        e,
                    )
                })?;
                let output_schema = self.get_output_schema(&tool_name);

                let mut artifact = McpToA2aTransformer::transform(
                    &crate::services::mcp::artifact_transformer::TransformParams {
                        tool_name: &tool_name,
                        tool_result: result,
                        output_schema,
                        context_id: &self.context_id,
                        task_id: &self.task_id,
                        tool_arguments: Some(&tool_call.arguments),
                    },
                )
                .map_err(|e| {
                    AgentServiceError::operation(
                        format!("Tool '{}' artifact transform failed", tool_call.name),
                        e,
                    )
                })?;

                artifact.metadata = artifact.metadata.with_execution_index(index);

                artifacts.push(artifact);
            }
        }

        Ok(artifacts)
    }
}
