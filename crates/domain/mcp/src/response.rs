use crate::repository::{CreateMcpArtifact, McpArtifactRepository};
use crate::schema::McpOutputSchema;
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, Content};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value as JsonValue;
use systemprompt_identifiers::{ArtifactId, McpExecutionId};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::{ExecutionMetadata, ToolResponse};

pub struct McpResponseBuilder<T: Serialize + JsonSchema> {
    output: T,
    tool_name: String,
    ctx: RequestContext,
    mcp_execution_id: McpExecutionId,
}

impl<T: Serialize + JsonSchema> std::fmt::Debug for McpResponseBuilder<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpResponseBuilder")
            .field("tool_name", &self.tool_name)
            .field("mcp_execution_id", &self.mcp_execution_id)
            .finish_non_exhaustive()
    }
}

impl<T: Serialize + JsonSchema + McpOutputSchema> McpResponseBuilder<T> {
    pub fn new(
        output: T,
        tool_name: impl Into<String>,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> Self {
        Self {
            output,
            tool_name: tool_name.into(),
            ctx: ctx.clone(),
            mcp_execution_id: exec_id.clone(),
        }
    }

    pub async fn build(
        self,
        summary: impl Into<String>,
        repo: &McpArtifactRepository,
        artifact_type: impl Into<String>,
        title: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        let artifact_id = ArtifactId::generate();
        let summary_str = summary.into();
        let artifact_type_str = artifact_type.into();
        let tool_name = self.tool_name;
        let exec_id = self.mcp_execution_id;

        let metadata = ExecutionMetadata::builder(&self.ctx)
            .with_tool(tool_name.clone())
            .with_execution(exec_id.to_string())
            .build();

        let meta_for_artifact = metadata.to_meta().map(|m| JsonValue::Object(m.0));
        let meta_for_result = metadata.to_meta();

        let tool_response =
            ToolResponse::new(artifact_id.clone(), exec_id.clone(), self.output, metadata);

        let structured_content = tool_response.to_json().map_err(|e| {
            tracing::error!(error = %e, tool = %tool_name, "Failed to serialize tool response");
            McpError::internal_error(format!("Serialization error: {e}"), None)
        })?;

        let output_schema = T::validated_schema();
        if let Some(content_obj) = structured_content.as_object() {
            let content_keys: Vec<&String> = content_obj.keys().collect();

            if let Some(schema_props) = output_schema.get("properties").and_then(|p| p.as_object())
            {
                let schema_keys: Vec<&String> = schema_props.keys().collect();
                let extra_keys: Vec<&&String> = content_keys
                    .iter()
                    .filter(|k| !schema_props.contains_key(k.as_str()))
                    .collect();

                if !extra_keys.is_empty() {
                    tracing::error!(
                        tool = %tool_name,
                        ?content_keys,
                        ?schema_keys,
                        ?extra_keys,
                        "structured_content has keys not in output_schema"
                    );
                }

                tracing::debug!(
                    tool = %tool_name,
                    artifact_id = %artifact_id,
                    ?content_keys,
                    schema_valid = extra_keys.is_empty(),
                    "MCP response validation"
                );
            } else {
                tracing::debug!(
                    tool = %tool_name,
                    artifact_id = %artifact_id,
                    ?content_keys,
                    "MCP response built (no schema properties to validate against)"
                );
            }
        }

        let create_artifact = CreateMcpArtifact {
            artifact_id: artifact_id.clone(),
            mcp_execution_id: exec_id,
            context_id: (!self.ctx.context_id().is_empty())
                .then(|| self.ctx.context_id().to_string()),
            user_id: (!self.ctx.user_id().is_anonymous()).then(|| self.ctx.user_id().to_string()),
            server_name: tool_name,
            artifact_type: artifact_type_str,
            title,
            data: structured_content.clone(),
            metadata: meta_for_artifact,
            expires_at: None,
        };

        repo.save(&create_artifact).await.map_err(|e| {
            tracing::error!(error = %e, artifact_id = %artifact_id, "Failed to persist artifact");
            McpError::internal_error(format!("Failed to persist artifact: {e}"), None)
        })?;

        tracing::info!(artifact_id = %artifact_id, server = %create_artifact.server_name, "Artifact persisted");

        let mut result = CallToolResult::success(vec![Content::text(summary_str)]);
        result.structured_content = Some(structured_content);
        if let Some(meta) = meta_for_result {
            result = result.with_meta(Some(meta));
        }
        Ok(result)
    }
}

impl<T: Serialize + JsonSchema> McpResponseBuilder<T> {
    pub fn build_error(error_message: impl Into<String>) -> CallToolResult {
        let error_text = error_message.into();

        CallToolResult::error(vec![Content::text(error_text)])
    }
}
