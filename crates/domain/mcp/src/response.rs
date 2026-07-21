//! MCP tool-response assembly with output-schema validation logging.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::repository::{CreateMcpArtifact, McpArtifactRepository};
use crate::schema::McpOutputSchema;
use crate::services::ui_renderer::{
    RenderTarget, UiResource, artifact_resource_uri, artifact_ui_resource,
};
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, ContentBlock, Meta, ResourceContents};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value as JsonValue;
use systemprompt_identifiers::{ArtifactId, McpExecutionId};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::{ExecutionMetadata, ToolResponse};
use systemprompt_models::mcp::McpResourceUiMeta;

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

        log_schema_validation(
            &tool_name,
            &artifact_id,
            &structured_content,
            &T::validated_schema(),
        );

        let create_artifact = CreateMcpArtifact {
            artifact_id: artifact_id.clone(),
            mcp_execution_id: exec_id,
            context_id: Some(self.ctx.context_id().clone()),
            user_id: (!self.ctx.is_anonymous()).then(|| self.ctx.user_id().clone()),
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

        let mut content = vec![ContentBlock::text(summary_str)];
        if let Some(block) = ui_resource_block(&create_artifact, &self.ctx).await {
            content.push(block);
        }

        let mut result = CallToolResult::success(content);
        result.structured_content = Some(structured_content);
        if let Some(meta) = meta_for_result {
            result = result.with_meta(Some(meta));
        }
        Ok(result)
    }
}

/// Renders the artifact to HTML and returns it as an embedded `ui://`
/// resource block. Rendering is presentational: a failure is logged and the
/// tool result goes out without it rather than failing the call.
async fn ui_resource_block(
    artifact: &CreateMcpArtifact,
    ctx: &RequestContext,
) -> Option<ContentBlock> {
    let payload = artifact.data.get("artifact")?;
    let target = RenderTarget {
        artifact_id: &artifact.artifact_id,
        artifact_type: &artifact.artifact_type,
        payload,
        context_id: ctx.context_id().clone(),
        title: artifact.title.clone(),
    };

    let resource = match artifact_ui_resource(&target).await {
        Ok(resource) => resource,
        Err(e) => {
            tracing::warn!(
                error = %e,
                artifact_id = %artifact.artifact_id,
                artifact_type = %artifact.artifact_type,
                "Artifact UI rendering failed; returning result without embedded resource"
            );
            return None;
        },
    };

    let ui_meta = McpResourceUiMeta::new()
        .with_prefers_border(true)
        .with_csp_opt(Some(resource.csp.to_mcp_domains()));

    Some(ContentBlock::resource(
        ResourceContents::TextResourceContents {
            uri: artifact_resource_uri(&artifact.server_name, &artifact.artifact_id),
            mime_type: Some(UiResource::mime_type().to_owned()),
            text: resource.html,
            meta: Some(Meta(ui_meta.to_meta_map())),
        },
    ))
}

fn log_schema_validation(
    tool_name: &str,
    artifact_id: &ArtifactId,
    structured_content: &JsonValue,
    output_schema: &JsonValue,
) {
    let Some(content_obj) = structured_content.as_object() else {
        return;
    };
    let content_keys: Vec<&String> = content_obj.keys().collect();

    let Some(schema_props) = output_schema.get("properties").and_then(|p| p.as_object()) else {
        tracing::debug!(
            tool = %tool_name,
            artifact_id = %artifact_id,
            ?content_keys,
            "MCP response built (no schema properties to validate against)"
        );
        return;
    };

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
}

impl<T: Serialize + JsonSchema> McpResponseBuilder<T> {
    pub fn build_error(error_message: impl Into<String>) -> CallToolResult {
        let error_text = error_message.into();

        CallToolResult::error(vec![ContentBlock::text(error_text)])
    }
}
