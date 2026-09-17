//! Wire shaping of a persisted artifact for the negotiated client profile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::model::{CallToolResult, ContentBlock, MetaObject, ResourceContents};
use serde_json::Value as JsonValue;
use systemprompt_identifiers::{ArtifactId, McpExecutionId};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::{EXECUTION_META_KEY, ExecutionMetadata};
use systemprompt_models::mcp::{ClientProfile, McpResourceUiMeta};

use super::UI_RESOURCE_URI_META_KEY;
use crate::services::ui_renderer::{
    RenderTarget, UiResource, artifact_resource_uri, artifact_ui_resource,
};

pub(super) struct RenderedArtifact {
    pub(super) artifact_id: ArtifactId,
    pub(super) mcp_execution_id: McpExecutionId,
    pub(super) server_name: String,
    pub(super) artifact_type: String,
    pub(super) title: Option<String>,
    // JSON: the stored (scanned, redacted) artifact body.
    pub(super) payload: JsonValue,
}

pub(super) struct WireShape<'a> {
    pub(super) client: &'a ClientProfile,
    pub(super) summary: String,
    pub(super) text_body: Option<String>,
    pub(super) structured_output: JsonValue,
    pub(super) metadata: &'a ExecutionMetadata,
}

impl WireShape<'_> {
    pub(super) fn into_result(
        self,
        artifact: &RenderedArtifact,
        ctx: &RequestContext,
    ) -> CallToolResult {
        let include_ui = self.client.supports_ui();
        let include_structured = self.client.supports_structured_content();
        let uri = artifact_resource_uri(&artifact.server_name, &artifact.artifact_id);

        let mut content = vec![ContentBlock::text(
            self.text_block(include_ui, include_structured),
        )];
        if include_ui && let Some(block) = ui_resource_block(artifact, ctx, &uri) {
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

fn ui_resource_block(
    artifact: &RenderedArtifact,
    ctx: &RequestContext,
    uri: &str,
) -> Option<ContentBlock> {
    let target = RenderTarget {
        artifact_id: &artifact.artifact_id,
        artifact_type: &artifact.artifact_type,
        payload: &artifact.payload,
        context_id: ctx.context_id().clone(),
        title: artifact.title.clone(),
    };

    let resource = match artifact_ui_resource(&target) {
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
