//! Deciding what a tool result is before it is stored.
//!
//! A result is *structured* when an MCP server declared it so — it carries
//! `structuredContent`, the systemprompt execution `_meta` key, or a body that
//! names a known `x-artifact-type`. A builtin client tool's free-form response
//! (a file read, a shell transcript) is still ingested as a typed
//! `tool_result`, but it is not structured, and the Artifacts pages show
//! structured results by default.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::model::{CallToolResult, ContentBlock, ResourceContents};
use serde_json::Value as JsonValue;
use systemprompt_identifiers::{ArtifactId, McpExecutionId};
use systemprompt_models::artifacts::{
    ArtifactType, AudioArtifact, ChartArtifact, CopyPasteTextArtifact, DashboardArtifact,
    EXECUTION_META_KEY, ImageArtifact, ListArtifact, MessageArtifact, PresentationCardArtifact,
    TableArtifact, TextArtifact, ToolResultArtifact, ToolResultBlock, VideoArtifact,
    payload_digest,
};

use super::IngestRequest;

#[derive(Debug, Clone)]
pub struct Classified {
    pub artifact_type: String,
    // JSON: the typed body, shaped per `artifact_type`.
    pub body: JsonValue,
    pub title: Option<String>,
    pub is_structured: bool,
    pub has_ui_resource: bool,
    pub is_error: bool,
    pub meta_artifact_id: Option<ArtifactId>,
    pub meta_execution_id: Option<McpExecutionId>,
}

impl Classified {
    /// The body kept when the real one exceeded the ceiling: the same shape,
    /// no content.
    // JSON: a `tool_result` body with `truncated: true` and no blocks.
    pub fn header_only(&self, request: &IngestRequest) -> JsonValue {
        let mut artifact =
            ToolResultArtifact::new(request.tool_name.clone()).with_error(self.is_error);
        artifact.server_name.clone_from(&request.server_name);
        artifact.truncated = true;
        serde_json::to_value(artifact).unwrap_or(JsonValue::Null)
    }
}

pub(super) fn classify(request: &IngestRequest) -> Classified {
    let result = &request.result;
    let (meta_artifact_id, meta_execution_id) = execution_meta(result);
    let has_meta = meta_artifact_id.is_some() || meta_execution_id.is_some();
    let is_error = result.is_error.unwrap_or(false);

    let blocks: Vec<ToolResultBlock> = result.content.iter().map(to_block).collect();
    let has_ui_resource = blocks.iter().any(ToolResultBlock::is_ui_resource);

    if let Some((artifact_type, body, title)) = typed_body(result.structured_content.as_ref()) {
        return Classified {
            artifact_type,
            body,
            title,
            is_structured: true,
            has_ui_resource,
            is_error,
            meta_artifact_id,
            meta_execution_id,
        };
    }

    let mut artifact = ToolResultArtifact::new(request.tool_name.clone())
        .with_error(is_error)
        .with_blocks(blocks)
        .with_structured_content(result.structured_content.clone());
    artifact.server_name.clone_from(&request.server_name);
    let is_structured = has_meta || result.structured_content.is_some();
    Classified {
        artifact_type: ToolResultArtifact::ARTIFACT_TYPE_STR.to_owned(),
        body: serde_json::to_value(artifact).unwrap_or(JsonValue::Null),
        title: None,
        is_structured,
        has_ui_resource,
        is_error,
        meta_artifact_id,
        meta_execution_id,
    }
}

fn execution_meta(result: &CallToolResult) -> (Option<ArtifactId>, Option<McpExecutionId>) {
    let Some(meta) = result.meta.as_ref() else {
        return (None, None);
    };
    let Some(exec) = meta
        .0
        .get(EXECUTION_META_KEY)
        .and_then(JsonValue::as_object)
    else {
        return (None, None);
    };
    let artifact_id = exec
        .get("artifact_id")
        .and_then(JsonValue::as_str)
        .filter(|s| !s.is_empty())
        .map(|s| ArtifactId::new(s.to_owned()));
    let execution_id = exec
        .get("mcp_execution_id")
        .and_then(JsonValue::as_str)
        .filter(|s| !s.is_empty())
        .map(|s| McpExecutionId::new(s.to_owned()));
    (artifact_id, execution_id)
}

fn typed_body(structured: Option<&JsonValue>) -> Option<(String, JsonValue, Option<String>)> {
    let value = structured?;
    let declared = value.get("x-artifact-type")?.as_str()?;
    let artifact_type: ArtifactType =
        serde_json::from_value(JsonValue::String(declared.to_owned())).ok()?;
    let title = value
        .get("title")
        .and_then(JsonValue::as_str)
        .map(str::to_owned);
    let body = typed_round_trip(&artifact_type, value).unwrap_or_else(|| {
        tracing::warn!(
            artifact_type = %artifact_type,
            "structured body declares a known artifact type but does not deserialise as it; stored as declared"
        );
        value.clone()
    });
    Some((artifact_type.to_string(), body, title))
}

// JSON: the declared type's canonical serialisation, when the body is one.
fn typed_round_trip(artifact_type: &ArtifactType, value: &JsonValue) -> Option<JsonValue> {
    match artifact_type {
        ArtifactType::Text => round_trip::<TextArtifact>(value),
        ArtifactType::Table => round_trip::<TableArtifact>(value),
        ArtifactType::Chart => round_trip::<ChartArtifact>(value),
        ArtifactType::Dashboard => round_trip::<DashboardArtifact>(value),
        ArtifactType::PresentationCard => round_trip::<PresentationCardArtifact>(value),
        ArtifactType::List => round_trip::<ListArtifact>(value),
        ArtifactType::Message => round_trip::<MessageArtifact>(value),
        ArtifactType::CopyPasteText => round_trip::<CopyPasteTextArtifact>(value),
        ArtifactType::Image => round_trip::<ImageArtifact>(value),
        ArtifactType::Video => round_trip::<VideoArtifact>(value),
        ArtifactType::Audio => round_trip::<AudioArtifact>(value),
        ArtifactType::Form | ArtifactType::Custom(_) => Some(value.clone()),
        ArtifactType::ToolResult => round_trip::<ToolResultArtifact>(value),
    }
}

// JSON: proves the declared body deserialises as its type, then stores the
// re-serialised typed form.
fn round_trip<T: serde::de::DeserializeOwned + serde::Serialize>(
    value: &JsonValue,
) -> Option<JsonValue> {
    let typed: T = serde_json::from_value(value.clone()).ok()?;
    serde_json::to_value(typed).ok()
}

fn to_block(block: &ContentBlock) -> ToolResultBlock {
    match block {
        ContentBlock::Text(text) => ToolResultBlock::Text {
            text: text.text.clone(),
        },
        ContentBlock::Image(image) => {
            let (byte_len, sha256) = blob_digest(&image.data);
            ToolResultBlock::Image {
                mime_type: image.mime_type.clone(),
                byte_len,
                sha256,
            }
        },
        ContentBlock::Audio(audio) => {
            let (byte_len, sha256) = blob_digest(&audio.data);
            ToolResultBlock::Audio {
                mime_type: audio.mime_type.clone(),
                byte_len,
                sha256,
            }
        },
        ContentBlock::Resource(embedded) => match &embedded.resource {
            ResourceContents::TextResourceContents {
                uri,
                mime_type,
                text,
                ..
            } => ToolResultBlock::Resource {
                uri: uri.clone(),
                mime_type: mime_type.clone(),
                text: Some(text.clone()),
                blob_byte_len: None,
                blob_sha256: None,
            },
            ResourceContents::BlobResourceContents {
                uri,
                mime_type,
                blob,
                ..
            } => {
                let (byte_len, sha256) = blob_digest(blob);
                ToolResultBlock::Resource {
                    uri: uri.clone(),
                    mime_type: mime_type.clone(),
                    text: None,
                    blob_byte_len: Some(byte_len),
                    blob_sha256: Some(sha256),
                }
            },
            _ => ToolResultBlock::Resource {
                uri: String::new(),
                mime_type: None,
                text: None,
                blob_byte_len: None,
                blob_sha256: None,
            },
        },
        ContentBlock::ResourceLink(link) => ToolResultBlock::ResourceLink {
            uri: link.uri.clone(),
            name: link.name.clone(),
            mime_type: link.mime_type.clone(),
        },
        _ => ToolResultBlock::Text {
            text: String::new(),
        },
    }
}

fn blob_digest(data: &str) -> (u64, String) {
    let digest = payload_digest(&JsonValue::String(data.to_owned()));
    (data.len() as u64, digest.sha256)
}
