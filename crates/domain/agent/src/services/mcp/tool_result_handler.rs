use anyhow::{anyhow, Result};
use std::fmt;
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::ai::tools::CallToolResult;

use super::artifact_transformer::McpToA2aTransformer;

#[derive(Clone, Copy)]
pub struct ToolResultHandler;

impl fmt::Debug for ToolResultHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToolResultHandler").finish_non_exhaustive()
    }
}

impl ToolResultHandler {
    pub const fn new() -> Self {
        Self
    }

    pub async fn process_tool_result(
        &self,
        tool_name: &str,
        tool_result: &CallToolResult,
        output_schema: Option<&serde_json::Value>,
        tool_arguments: Option<&serde_json::Value>,
        task_id: &TaskId,
        context_id: &ContextId,
        context: &systemprompt_models::RequestContext,
    ) -> Result<crate::models::a2a::Artifact> {
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

        let artifact = McpToA2aTransformer::transform(
            tool_name,
            tool_result,
            output_schema,
            context_id.as_str(),
            task_id.as_str(),
            tool_arguments,
        )
        .map_err(|e| anyhow::anyhow!("Artifact transform failed: {}", e))?;

        use systemprompt_traits::validation::Validate;
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
