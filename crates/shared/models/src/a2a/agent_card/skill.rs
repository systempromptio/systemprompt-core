//! Card-level skill, transport, provider and signature descriptors.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::a2a::transport::ProtocolBinding;

/// One transport binding the agent supports.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentInterface {
    /// Public URL the transport listens on.
    pub url: String,
    /// Wire protocol (JSON-RPC, gRPC, …).
    pub protocol_binding: ProtocolBinding,
    /// A2A protocol version exposed.
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,
}

/// Provider attribution for an agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentProvider {
    /// Organization that publishes the agent.
    pub organization: String,
    /// Canonical organisation URL.
    pub url: String,
}

/// A2A skill descriptor as serialised on a public agent card.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkill {
    /// A2A spec skill identifier (string for protocol compatibility).
    pub id: String,
    /// Human-readable skill name.
    pub name: String,
    /// Free-form description.
    pub description: String,
    /// Tags for client-side filtering.
    pub tags: Vec<String>,
    /// Optional example invocations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<String>>,
    /// Optional acceptable input MIME types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_modes: Option<Vec<String>>,
    /// Optional produced MIME types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_modes: Option<Vec<String>>,
    /// Optional skill-scoped security requirements.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<Vec<HashMap<String, Vec<String>>>>,
}

impl AgentSkill {
    /// Construct an [`AgentSkill`] from MCP server metadata.
    #[must_use]
    pub const fn from_mcp_server(
        server_name: String,
        display_name: String,
        description: String,
        tags: Vec<String>,
    ) -> Self {
        Self {
            id: server_name,
            name: display_name,
            description,
            tags,
            examples: None,
            input_modes: None,
            output_modes: None,
            security: None,
        }
    }

    /// Borrow the underlying MCP server name (the skill's `id`).
    #[must_use]
    pub fn mcp_server_name(&self) -> &str {
        &self.id
    }
}

/// JWS-style signature block embedded on an agent card.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentCardSignature {
    /// Base64url-encoded protected header.
    pub protected: String,
    /// Base64url-encoded signature value.
    pub signature: String,
    /// Optional unprotected JOSE header.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<serde_json::Value>,
}

fn default_protocol_version() -> String {
    "1.0.0".to_string()
}
