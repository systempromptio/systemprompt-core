use anyhow::{Result, anyhow};
use std::fmt;
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::ai::tools::CallToolResult;
use systemprompt_traits::validation::Validate;

use super::artifact_transformer::McpToA2aTransformer;

#[derive(Debug)]
pub struct ProcessToolResultParams<'a> {
    pub tool_name: &'a str,
    pub tool_result: &'a CallToolResult,
    pub output_schema: Option<&'a serde_json::Value>,
    pub tool_arguments: Option<&'a serde_json::Value>,
    pub task_id: &'a TaskId,
    pub context_id: &'a ContextId,
    pub context: &'a systemprompt_models::RequestContext,
}

#[derive(Clone, Copy)]
pub struct ToolResultHandler;

impl fmt::Debug for ToolResultHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToolResultHandler").finish_non_exhaustive()
    }
}

impl Default for ToolResultHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolResultHandler {
    pub const fn new() -> Self {
        Self
    }

    pub fn process_tool_result(
        params: &ProcessToolResultParams<'_>,
    ) -> Result<crate::models::a2a::Artifact> {
        let ProcessToolResultParams {
            tool_name,
            tool_result,
            output_schema,
            tool_arguments,
            task_id,
            context_id,
            context,
        } = params;
        if !context.is_authenticated() || context.is_system() {
            return Err(anyhow!(
                "Invalid user - unauthenticated and system users cannot create artifacts"
            ));
        }

        tracing::info!(
            tool_name = %tool_name,
            task_id = %task_id,
            user_id = %context.user_id(),
            context_id = %context_id,
            "Transforming tool result to artifact"
        );

        let artifact =
            McpToA2aTransformer::transform(&super::artifact_transformer::TransformParams {
                tool_name,
                tool_result,
                output_schema: *output_schema,
                context_id: context_id.as_str(),
                task_id: task_id.as_str(),
                tool_arguments: *tool_arguments,
            })
            .map_err(|e| anyhow::anyhow!("Artifact transform failed: {}", e))?;

        artifact
            .metadata
            .validate()
            .map_err(|e| anyhow::anyhow!("Artifact metadata validation failed: {}", e))?;

        tracing::info!(
            artifact_id = %artifact.id,
            tool_name = %tool_name,
            user_id = %context.user_id(),
            task_id = %task_id,
            fingerprint = ?artifact.metadata.fingerprint,
            "Transformed tool result to artifact"
        );

        Ok(artifact)
    }
}
