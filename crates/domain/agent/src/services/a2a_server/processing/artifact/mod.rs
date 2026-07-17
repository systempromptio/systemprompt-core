//! Construction of A2A [`Artifact`]s from MCP tool results.
//!
//! [`ArtifactBuilder`] pairs each tool call with its structured result and
//! transforms it into an A2A artifact via [`McpToA2aTransformer`], skipping
//! results without structured content.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::{AgentServiceError, Result};
use systemprompt_identifiers::{ContextId, TaskId};
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

    fn get_output_schema(&self, tool_name: &str) -> Option<&serde_json::Value> {
        self.tools
            .iter()
            .find(|t| t.name == tool_name)
            .and_then(|t| t.output_schema.as_ref())
    }

    pub fn build_artifacts(&self) -> Result<Vec<Artifact>> {
        let mut artifacts = Vec::new();

        for (index, result) in self.tool_results.iter().enumerate() {
            if let Some(structured_content) =
                result.structured_content.as_ref().filter(|v| !v.is_null())
                && let Some(tool_call) = self.tool_calls.get(index)
            {
                let output_schema = self.get_output_schema(&tool_call.name);

                let mut artifact = McpToA2aTransformer::transform_from_json(
                    &crate::services::mcp::artifact_transformer::TransformFromJsonParams {
                        tool_name: &tool_call.name,
                        tool_result_json: structured_content,
                        output_schema,
                        context_id: self.context_id.as_str(),
                        task_id: self.task_id.as_str(),
                        tool_arguments: Some(&tool_call.arguments),
                    },
                )
                .map_err(|e| {
                    AgentServiceError::Internal(format!(
                        "Tool '{}' artifact transform failed: {e}",
                        tool_call.name
                    ))
                })?;

                artifact.metadata = artifact.metadata.with_execution_index(index);

                artifacts.push(artifact);
            }
        }

        Ok(artifacts)
    }
}
