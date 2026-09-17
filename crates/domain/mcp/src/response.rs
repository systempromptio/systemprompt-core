//! Client-aware MCP tool-response assembly.
//!
//! Every tool output is persisted as an artifact through the ingestion
//! narrow waist ([`ArtifactIngest`]) — the same path a proxied,
//! gateway-replayed or hook-reported result takes, so in-process output is
//! scanned, content- addressed and linked exactly like everything else — then
//! shaped for the wire according to the negotiated [`ClientProfile`]:
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

use crate::schema::McpOutputSchema;
use crate::services::artifact_ingest::{ArtifactIngest, IngestRequest};
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
use systemprompt_models::artifacts::{EXECUTION_META_KEY, ExecutionMetadata};
use systemprompt_models::mcp::{ClientProfile, ExecutionSource, McpResourceUiMeta};

pub const UI_RESOURCE_URI_META_KEY: &str = "io.systemprompt/ui-resource-uri";

#[derive(Clone, Debug)]
pub struct ToolIdentity {
    server_name: String,
    tool_name: String,
}

impl ToolIdentity {
    pub fn new(server_name: impl Into<String>, tool_name: impl Into<String>) -> Self {
        Self {
            server_name: server_name.into(),
            tool_name: tool_name.into(),
        }
    }
}

pub struct McpResponseBuilder<T: Serialize + JsonSchema> {
    output: T,
    identity: ToolIdentity,
    ctx: RequestContext,
    mcp_execution_id: McpExecutionId,
    client: ClientProfile,
}

impl<T: Serialize + JsonSchema> std::fmt::Debug for McpResponseBuilder<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpResponseBuilder")
            .field("identity", &self.identity)
            .field("mcp_execution_id", &self.mcp_execution_id)
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

impl<T: Serialize + JsonSchema + McpOutputSchema> McpResponseBuilder<T> {
    pub fn new(
        output: T,
        identity: ToolIdentity,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
        client: &ClientProfile,
    ) -> Self {
        Self {
            output,
            identity,
            ctx: ctx.clone(),
            mcp_execution_id: exec_id.clone(),
            client: client.clone(),
        }
    }

    pub async fn build(
        self,
        summary: impl Into<String>,
        ingest: &ArtifactIngest,
        artifact_type: impl Into<String>,
        title: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        let summary_str = summary.into();
        let artifact_type_str = artifact_type.into();
        let ToolIdentity {
            server_name,
            tool_name,
        } = self.identity;
        let exec_id = self.mcp_execution_id;

        let structured_output = serde_json::to_value(&self.output).map_err(|e| {
            tracing::error!(error = %e, tool = %tool_name, "Failed to serialize tool output");
            McpError::internal_error(format!("Serialization error: {e}"), None)
        })?;
        let text_body = self.output.text_body();

        let mut wire = CallToolResult::success(Vec::new());
        wire.structured_content = Some(typed_structured(&structured_output, &artifact_type_str));
        let outcome = ingest
            .ingest(IngestRequest {
                result: wire,
                tool_name: tool_name.clone(),
                server_name: Some(server_name.clone()),
                ai_tool_call_id: self.ctx.ai_tool_call_id().cloned(),
                mcp_execution_id: Some(exec_id.clone()),
                ctx: self.ctx.clone(),
                skill: None,
                source: ExecutionSource::InProcess,
                started_at: None,
                input: None,
            })
            .await
            .map_err(|e| {
                tracing::error!(error = %e, tool = %tool_name, "Failed to persist artifact");
                McpError::internal_error(format!("Failed to persist artifact: {e}"), None)
            })?;
        let artifact_id = outcome.artifact_id;

        let metadata = ExecutionMetadata::builder(&self.ctx)
            .with_tool(tool_name.clone())
            .with_execution(exec_id.to_string())
            .build();

        tracing::info!(artifact_id = %artifact_id, server = %server_name, "Artifact persisted");

        // Why: the embedded UI renders the stored body — the scanned, redacted
        // artifact — while `structuredContent` must stay the typed output the
        // advertised `outputSchema` describes: the ingest may re-shape the
        // body (a `tool_result` envelope around an unrecognised type), and a
        // client that validates the schema rejects that envelope. Only a
        // redaction substitutes the stored copy, unwrapped back to the typed
        // object, so a secret the scanner removed never reaches the model.
        let stored = ingest
            .artifacts()
            .find_by_id(&artifact_id)
            .await
            .ok()
            .flatten();
        let redacted = stored.as_ref().is_some_and(|r| r.secret_redactions > 0);
        let typed_output = typed_structured(&structured_output, &artifact_type_str);
        let stored_body = stored
            .and_then(|record| record.data.get("artifact").cloned())
            .unwrap_or_else(|| typed_output.clone());
        let wire_output = if redacted {
            unwrap_tool_result(&stored_body)
        } else {
            typed_output
        };
        let rendered = RenderedArtifact {
            artifact_id,
            mcp_execution_id: exec_id,
            server_name,
            artifact_type: artifact_type_str,
            title,
            payload: stored_body,
        };

        let shape = WireShape {
            client: &self.client,
            summary: summary_str,
            text_body: if redacted { None } else { text_body },
            structured_output: wire_output,
            metadata: &metadata,
        };
        Ok(shape.into_result(&rendered, &self.ctx))
    }
}

/// A body that names its own artifact type, so the ingest stores it as that
/// type rather than as a generic tool result.
// JSON: the typed output object with `x-artifact-type` set when absent.
fn typed_structured(output: &JsonValue, artifact_type: &str) -> JsonValue {
    let mut value = output.clone();
    if let Some(map) = value.as_object_mut()
        && !map.contains_key("x-artifact-type")
    {
        map.insert(
            "x-artifact-type".to_owned(),
            JsonValue::String(artifact_type.to_owned()),
        );
    }
    value
}

/// The typed object inside a stored `tool_result` envelope, or the body
/// itself when the ingest stored it as its declared type.
// JSON: the value under `structured_content` of a tool_result body, else the input.
fn unwrap_tool_result(stored: &JsonValue) -> JsonValue {
    let is_envelope = stored.get("x-artifact-type").and_then(JsonValue::as_str)
        == Some(systemprompt_models::artifacts::ToolResultArtifact::ARTIFACT_TYPE_STR);
    match stored.get("structured_content") {
        Some(inner) if is_envelope && !inner.is_null() => inner.clone(),
        _ => stored.clone(),
    }
}

/// What the wire shaper needs to know about the persisted artifact.
struct RenderedArtifact {
    artifact_id: ArtifactId,
    mcp_execution_id: McpExecutionId,
    server_name: String,
    artifact_type: String,
    title: Option<String>,
    // JSON: the stored (scanned, redacted) artifact body.
    payload: JsonValue,
}

struct WireShape<'a> {
    client: &'a ClientProfile,
    summary: String,
    text_body: Option<String>,
    structured_output: JsonValue,
    metadata: &'a ExecutionMetadata,
}

impl WireShape<'_> {
    fn into_result(self, artifact: &RenderedArtifact, ctx: &RequestContext) -> CallToolResult {
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

impl<T: Serialize + JsonSchema> McpResponseBuilder<T> {
    pub fn build_error(error_message: impl Into<String>) -> CallToolResult {
        let error_text = error_message.into();

        CallToolResult::error(vec![ContentBlock::text(error_text)])
    }
}
