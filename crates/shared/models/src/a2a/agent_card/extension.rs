//! Agent capability flags and the named extension catalogue.

use serde::{Deserialize, Serialize};

pub const ARTIFACT_RENDERING_URI: &str = "https://systemprompt.io/extensions/artifact-rendering/v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push_notifications: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_transition_history: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<AgentExtension>>,
}

impl Default for AgentCapabilities {
    fn default() -> Self {
        Self {
            streaming: Some(true),
            push_notifications: Some(true),
            state_transition_history: Some(true),
            extensions: None,
        }
    }
}

impl AgentCapabilities {
    #[must_use]
    pub const fn normalize(mut self) -> Self {
        if self.streaming.is_none() {
            self.streaming = Some(true);
        }
        if self.push_notifications.is_none() {
            self.push_notifications = Some(false);
        }
        if self.state_transition_history.is_none() {
            self.state_transition_history = Some(true);
        }
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentExtension {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl AgentExtension {
    #[must_use]
    pub fn mcp_tools_extension() -> Self {
        Self {
            uri: "systemprompt:mcp-tools".to_owned(),
            description: Some("MCP tool execution capabilities".to_owned()),
            required: Some(false),
            params: Some(serde_json::json!({
                "supported_protocols": ["mcp-1.0"]
            })),
        }
    }

    #[must_use]
    pub fn agent_identity(agent_name: &str) -> Self {
        Self {
            uri: "systemprompt:agent-identity".to_owned(),
            description: Some("systemprompt.io platform agent name".to_owned()),
            required: Some(true),
            params: Some(serde_json::json!({
                "name": agent_name
            })),
        }
    }

    #[must_use]
    pub fn system_instructions(system_prompt: &str) -> Self {
        Self {
            uri: "systemprompt:system-instructions".to_owned(),
            description: Some("Agent system prompt and behavioral guidelines".to_owned()),
            required: Some(true),
            params: Some(serde_json::json!({
                "systemPrompt": system_prompt,
                "format": "text/plain"
            })),
        }
    }

    #[must_use]
    pub fn system_instructions_opt(system_prompt: Option<&str>) -> Option<Self> {
        system_prompt.map(Self::system_instructions)
    }

    #[must_use]
    pub fn service_status(
        status: &str,
        port: Option<u16>,
        pid: Option<u32>,
        default: bool,
    ) -> Self {
        let mut params = serde_json::json!({
            "status": status,
            "default": default
        });

        if let Some(p) = port {
            params["port"] = serde_json::json!(p);
        }
        if let Some(p) = pid {
            params["pid"] = serde_json::json!(p);
        }

        Self {
            uri: "systemprompt:service-status".to_owned(),
            description: Some("Runtime service status from orchestrator".to_owned()),
            required: Some(true),
            params: Some(params),
        }
    }
}
