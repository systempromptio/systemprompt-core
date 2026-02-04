use crate::error::McpError;
use crate::repository::{CreateMcpArtifact, McpArtifactRepository};
use rmcp::model::{CallToolResult, Content};
use schemars::JsonSchema;
use serde::Serialize;
use systemprompt_identifiers::{ArtifactId, McpExecutionId};
use systemprompt_models::artifacts::{ExecutionMetadata, ToolResponse};
use systemprompt_models::RequestContext;

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

impl<T: Serialize + JsonSchema> McpResponseBuilder<T> {
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

    pub fn build(self, summary: impl Into<String>) -> Result<CallToolResult, McpError> {
        let artifact_id = ArtifactId::generate();

        let metadata = ExecutionMetadata::builder(&self.ctx)
            .with_tool(self.tool_name.clone())
            .with_execution(self.mcp_execution_id.to_string())
            .build();

        let tool_response = ToolResponse::new(
            artifact_id,
            self.mcp_execution_id.clone(),
            self.output,
            metadata.clone(),
        );

        let structured_content = tool_response.to_json().map_err(|e| {
            tracing::error!(error = %e, tool = %self.tool_name, "Failed to serialize tool response");
            McpError::Serialization(e)
        })?;

        Ok(CallToolResult {
            content: vec![Content::text(summary.into())],
            structured_content: Some(structured_content),
            is_error: Some(false),
            meta: metadata.to_meta(),
        })
    }

    pub async fn build_and_persist(
        self,
        summary: impl Into<String>,
        repo: &McpArtifactRepository,
        artifact_type: impl Into<String>,
        title: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        let artifact_id = ArtifactId::generate();
        let summary_str = summary.into();
        let artifact_type_str = artifact_type.into();

        let metadata = ExecutionMetadata::builder(&self.ctx)
            .with_tool(self.tool_name.clone())
            .with_execution(self.mcp_execution_id.to_string())
            .build();

        let tool_response = ToolResponse::new(
            artifact_id.clone(),
            self.mcp_execution_id.clone(),
            self.output,
            metadata.clone(),
        );

        let structured_content = tool_response.to_json().map_err(|e| {
            tracing::error!(error = %e, tool = %self.tool_name, "Failed to serialize tool response");
            McpError::Serialization(e)
        })?;

        let create_artifact = CreateMcpArtifact {
            artifact_id: artifact_id.clone(),
            mcp_execution_id: self.mcp_execution_id.clone(),
            context_id: (!self.ctx.context_id().is_empty())
                .then(|| self.ctx.context_id().to_string()),
            user_id: (!self.ctx.user_id().is_anonymous()).then(|| self.ctx.user_id().to_string()),
            server_name: self.tool_name.clone(),
            artifact_type: artifact_type_str,
            title,
            data: structured_content.clone(),
            metadata: metadata
                .to_meta()
                .and_then(|m| serde_json::to_value(m.0).ok()),
            expires_at: None,
        };

        repo.save(&create_artifact).await.map_err(|e| {
            tracing::error!(error = %e, artifact_id = %artifact_id, "Failed to persist artifact");
            McpError::Internal(format!("Failed to persist artifact: {e}"))
        })?;

        tracing::info!(artifact_id = %artifact_id, server = %self.tool_name, "Artifact persisted");

        Ok(CallToolResult {
            content: vec![Content::text(summary_str)],
            structured_content: Some(structured_content),
            is_error: Some(false),
            meta: metadata.to_meta(),
        })
    }

    pub fn build_error(error_message: impl Into<String>) -> CallToolResult {
        let error_text = error_message.into();

        CallToolResult {
            content: vec![Content::text(error_text)],
            structured_content: None,
            is_error: Some(true),
            meta: None,
        }
    }
}
