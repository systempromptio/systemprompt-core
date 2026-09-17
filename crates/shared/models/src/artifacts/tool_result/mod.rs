//! Tool-result artifact: the typed form of an MCP `CallToolResult` as the
//! model saw it.
//!
//! Every tool result that reaches the platform — from an in-process server, a
//! proxied external server, a gateway `tool_result` block, or a client hook —
//! is normalised into a [`ToolResultArtifact`] so that one artifact model
//! covers every source. The MCP result is itself a typed wire schema (content
//! blocks, optional structured content, error flag), so mirroring it here is a
//! typed model, not a JSON escape hatch: binary block bytes are never stored,
//! only their length and digest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};

use crate::artifacts::traits::Artifact;
use crate::artifacts::types::ArtifactType;

fn default_artifact_type() -> String {
    ToolResultArtifact::ARTIFACT_TYPE_STR.to_owned()
}

/// One content block of a tool result, with binary payloads reduced to their
/// size and digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultBlock {
    Text {
        text: String,
    },
    Image {
        mime_type: String,
        byte_len: u64,
        sha256: String,
    },
    Audio {
        mime_type: String,
        byte_len: u64,
        sha256: String,
    },
    Resource {
        uri: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        blob_byte_len: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        blob_sha256: Option<String>,
    },
    ResourceLink {
        uri: String,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
}

impl ToolResultBlock {
    #[must_use]
    pub fn is_ui_resource(&self) -> bool {
        match self {
            Self::Resource { uri, .. } | Self::ResourceLink { uri, .. } => uri.starts_with("ui://"),
            Self::Text { .. } | Self::Image { .. } | Self::Audio { .. } => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ToolResultArtifact {
    #[serde(rename = "x-artifact-type")]
    #[serde(default = "default_artifact_type")]
    pub artifact_type: String,
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    #[serde(default)]
    pub blocks: Vec<ToolResultBlock>,
    // JSON: MCP `structuredContent` is the tool's own output-schema object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<JsonValue>,
}

impl ToolResultArtifact {
    pub const ARTIFACT_TYPE_STR: &'static str = "tool_result";

    #[must_use]
    pub fn new(tool_name: impl Into<String>) -> Self {
        Self {
            artifact_type: default_artifact_type(),
            tool_name: tool_name.into(),
            server_name: None,
            is_error: false,
            truncated: false,
            blocks: Vec::new(),
            structured_content: None,
        }
    }

    #[must_use]
    pub fn with_server(mut self, server_name: impl Into<String>) -> Self {
        self.server_name = Some(server_name.into());
        self
    }

    #[must_use]
    pub const fn with_error(mut self, is_error: bool) -> Self {
        self.is_error = is_error;
        self
    }

    #[must_use]
    pub fn with_blocks(mut self, blocks: Vec<ToolResultBlock>) -> Self {
        self.blocks = blocks;
        self
    }

    #[must_use]
    pub fn with_structured_content(mut self, value: Option<JsonValue>) -> Self {
        self.structured_content = value;
        self
    }

    #[must_use]
    pub fn has_ui_resource(&self) -> bool {
        self.blocks.iter().any(ToolResultBlock::is_ui_resource)
    }
}

impl Artifact for ToolResultArtifact {
    fn artifact_type(&self) -> ArtifactType {
        ArtifactType::ToolResult
    }

    // JSON: JSON Schema document describing the artifact for the model.
    fn to_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "tool_name": { "type": "string" },
                "server_name": { "type": "string" },
                "is_error": { "type": "boolean" },
                "blocks": { "type": "array", "items": { "type": "object" } },
                "structured_content": { "type": "object" }
            },
            "required": ["tool_name", "blocks"],
            "x-artifact-type": Self::ARTIFACT_TYPE_STR
        })
    }
}
