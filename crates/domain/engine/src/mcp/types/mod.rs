//! The MCP values this client hands to callers, read leniently from server
//! JSON.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::client::SUPPORTED_PROTOCOL_VERSIONS;
use crate::error::McpError;

mod content;

pub use content::{Attachment, CallResult, PromptMessage, PromptResult};

/// How this client introduces itself in `initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl ClientInfo {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            title: None,
        }
    }
}

/// What a server reported about itself in `initialize`.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerInfo {
    pub protocol_version: String,
    pub name: String,
    pub version: String,
    pub instructions: Option<String>,
    // JSON: the server's capability object, open-ended by the MCP specification.
    pub capabilities: Value,
}

impl ServerInfo {
    // JSON: the `initialize` result as the server sent it.
    pub(crate) fn from_initialize(result: &Value) -> Result<Self, McpError> {
        let protocol_version = result
            .get("protocolVersion")
            .and_then(Value::as_str)
            .ok_or(McpError::Protocol(
                "initialize result has no protocol version",
            ))?;
        if !SUPPORTED_PROTOCOL_VERSIONS.contains(&protocol_version) {
            return Err(McpError::ProtocolVersion(protocol_version.to_owned()));
        }
        let info = result.get("serverInfo");
        let field = |key: &str| {
            info.and_then(|info| info.get(key))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        Ok(Self {
            protocol_version: protocol_version.to_owned(),
            name: field("name"),
            version: field("version"),
            instructions: result
                .get("instructions")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_owned),
            capabilities: result
                .get("capabilities")
                .cloned()
                .unwrap_or(Value::Object(Map::new())),
        })
    }

    pub fn has_capability(&self, name: &str) -> bool {
        self.capabilities
            .get(name)
            .is_some_and(|value| !value.is_null())
    }
}

/// A tool as the server lists it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "empty_object_schema")]
    pub input_schema: Value,
}

// JSON: the JSON Schema for a tool that takes no arguments.
pub(crate) fn empty_object_schema() -> Value {
    let mut schema = Map::new();
    schema.insert("type".into(), Value::from("object"));
    schema.insert("properties".into(), Value::Object(Map::new()));
    Value::Object(schema)
}

/// A resource as the server lists it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    pub uri: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
}

/// One item of a `resources/read` result: `text` or base64 `blob`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceContent {
    pub uri: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub blob: Option<String>,
}

/// A prompt template as the server lists it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub arguments: Vec<PromptArgument>,
}

/// One argument a prompt template takes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptArgument {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
}
