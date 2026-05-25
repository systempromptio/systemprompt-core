//! Card-level skill, transport, provider and signature descriptors.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::a2a::transport::ProtocolBinding;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentInterface {
    pub url: String,
    pub protocol_binding: ProtocolBinding,
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentProvider {
    pub organization: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_modes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_modes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<Vec<HashMap<String, Vec<String>>>>,
}

impl AgentSkill {
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

    #[must_use]
    pub fn mcp_server_name(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentCardSignature {
    pub protected: String,
    pub signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<serde_json::Value>,
}

fn default_protocol_version() -> String {
    "1.0.0".to_owned()
}
