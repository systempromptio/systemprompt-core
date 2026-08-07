//! Client-aware MCP tool-response assembly.
//!
//! Every tool output is persisted as an artifact, then shaped for the wire
//! according to the negotiated [`ClientProfile`]:
//!
//! - Hosts that negotiated the MCP Apps UI extension receive the embedded
//!   `ui://` resource and [`UI_RESOURCE_URI_META_KEY`] alongside the text
//!   summary.
//! - Clients on protocol `2025-06-18` or later receive `structuredContent`
//!   holding the tool's typed output directly, matching the advertised output
//!   schema.
//! - Everything else — including clients whose declaration is unknown —
//!   receives only text content, with the artifact body folded into the text
//!   block so the data still arrives.
//!
//! Execution provenance travels under the single reverse-DNS `_meta` key
//! [`EXECUTION_META_KEY`](systemprompt_models::artifacts::EXECUTION_META_KEY);
//! MCP reserves unprefixed `_meta` keys, so no bare field ever reaches the
//! wire. Rendering is presentational: a renderer failure drops the embedded
//! resource rather than failing the tool call.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::repository::{CreateMcpArtifact, McpArtifactRepository};
use crate::schema::McpOutputSchema;
use crate::services::ui_renderer::{
    RenderTarget, UiResource, artifact_resource_uri, artifact_ui_resource,
};
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, ContentBlock, MetaObject, ResourceContents};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value as JsonValue;
use systemprompt_identifiers::{ArtifactId, McpExecutionId};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::{EXECUTION_META_KEY, ExecutionMetadata, ToolResponse};
use systemprompt_models::mcp::{ClientProfile, McpResourceUiMeta};

pub const UI_RESOURCE_URI_META_KEY: &str = "io.systemprompt/ui-resource-uri";

pub struct McpResponseBuilder<T: Serialize + JsonSchema> {
    output: T,
    tool_name: String,
    ctx: RequestContext,
    mcp_execution_id: McpExecutionId,
    client: ClientProfile,
}

impl<T: Serialize + JsonSchema> std::fmt::Debug for McpResponseBuilder<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpResponseBuilder")
            .field("tool_name", &self.tool_name)
            .field("mcp_execution_id", &self.mcp_execution_id)
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

impl<T: Serialize + JsonSchema + McpOutputSchema> McpResponseBuilder<T> {
    pub fn new(
        output: T,
        tool_name: impl Into<String>,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
        client: &ClientProfile,
    ) -> Self {
        Self {
            output,
            tool_name: tool_name.into(),
            ctx: ctx.clone(),
            mcp_execution_id: exec_id.clone(),
            client: client.clone(),
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

        let structured_output = serde_json::to_value(&self.output).map_err(|e| {
            tracing::error!(error = %e, tool = %tool_name, "Failed to serialize tool output");
            McpError::internal_error(format!("Serialization error: {e}"), None)
        })?;
        let text_body = self.output.text_body();

        let stored_envelope = ToolResponse::new(
            artifact_id.clone(),
            exec_id.clone(),
            self.output,
            metadata.clone(),
        )
        .to_json()
        .map_err(|e| {
            tracing::error!(error = %e, tool = %tool_name, "Failed to serialize tool response");
            McpError::internal_error(format!("Serialization error: {e}"), None)
        })?;

        let create_artifact = CreateMcpArtifact {
            artifact_id: artifact_id.clone(),
            mcp_execution_id: exec_id.clone(),
            context_id: Some(self.ctx.context_id().clone()),
            user_id: (!self.ctx.is_anonymous()).then(|| self.ctx.user_id().clone()),
            server_name: tool_name,
            artifact_type: artifact_type_str,
            title,
            data: stored_envelope,
            metadata: metadata.to_object().map(JsonValue::Object),
            expires_at: None,
        };

        repo.save(&create_artifact).await.map_err(|e| {
            tracing::error!(error = %e, artifact_id = %artifact_id, "Failed to persist artifact");
            McpError::internal_error(format!("Failed to persist artifact: {e}"), None)
        })?;

        tracing::info!(artifact_id = %artifact_id, server = %create_artifact.server_name, "Artifact persisted");

        let shape = WireShape {
            client: &self.client,
            summary: summary_str,
            text_body,
            structured_output,
            metadata: &metadata,
        };
        Ok(shape.into_result(&create_artifact, &self.ctx).await)
    }
}

struct WireShape<'a> {
    client: &'a ClientProfile,
    summary: String,
    text_body: Option<String>,
    structured_output: JsonValue,
    metadata: &'a ExecutionMetadata,
}

impl WireShape<'_> {
    async fn into_result(
        self,
        artifact: &CreateMcpArtifact,
        ctx: &RequestContext,
    ) -> CallToolResult {
        let include_ui = self.client.supports_ui();
        let include_structured = self.client.supports_structured_content();
        let uri = artifact_resource_uri(&artifact.server_name, &artifact.artifact_id);

        let mut content = vec![ContentBlock::text(self.text_block(
            include_ui,
            include_structured,
        ))];
        if include_ui && let Some(block) = ui_resource_block(artifact, ctx, &uri).await {
            content.push(block);
        }

        let mut result = CallToolResult::success(content);
        if include_structured {
            result.structured_content = Some(self.structured_output);
        }
        if include_ui || include_structured {
            result = result.with_meta(wire_meta(
                self.metadata,
                &artifact.artifact_id,
                &artifact.mcp_execution_id,
                &uri,
            ));
        }
        result
    }

    fn text_block(&self, include_ui: bool, include_structured: bool) -> String {
        let summary = &self.summary;
        if include_ui || (include_structured && self.text_body.is_none()) {
            return summary.clone();
        }
        if let Some(body) = &self.text_body {
            return format!("{summary}\n\n{body}");
        }
        let pretty = serde_json::to_string_pretty(&self.structured_output)
            .unwrap_or_else(|_| self.structured_output.to_string());
        format!("{summary}\n\n```json\n{pretty}\n```")
    }
}

fn wire_meta(
    metadata: &ExecutionMetadata,
    artifact_id: &ArtifactId,
    exec_id: &McpExecutionId,
    ui_resource_uri: &str,
) -> Option<MetaObject> {
    let mut fields = metadata.to_object()?;
    fields.insert(
        "artifact_id".to_owned(),
        JsonValue::String(artifact_id.to_string()),
    );
    fields.insert(
        "mcp_execution_id".to_owned(),
        JsonValue::String(exec_id.to_string()),
    );

    let mut meta = serde_json::Map::new();
    meta.insert(EXECUTION_META_KEY.to_owned(), JsonValue::Object(fields));
    meta.insert(
        UI_RESOURCE_URI_META_KEY.to_owned(),
        JsonValue::String(ui_resource_uri.to_owned()),
    );
    Some(MetaObject(meta))
}

async fn ui_resource_block(
    artifact: &CreateMcpArtifact,
    ctx: &RequestContext,
    uri: &str,
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
            uri: uri.to_owned(),
            mime_type: Some(UiResource::mime_type().to_owned()),
            text: resource.html,
            meta: Some(MetaObject(ui_meta.to_meta_map())),
        },
    ))
}

impl<T: Serialize + JsonSchema> McpResponseBuilder<T> {
    pub fn build_error(error_message: impl Into<String>) -> CallToolResult {
        let error_text = error_message.into();

        CallToolResult::error(vec![ContentBlock::text(error_text)])
    }
}
