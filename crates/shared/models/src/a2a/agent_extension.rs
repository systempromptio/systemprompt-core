use serde::{Deserialize, Serialize};

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
    pub fn mcp_tools_extension() -> Self {
        Self {
            uri: "systemprompt:mcp-tools".to_string(),
            description: Some("MCP tool execution capabilities".to_string()),
            required: Some(false),
            params: Some(serde_json::json!({
                "supported_protocols": ["mcp-1.0"]
            })),
        }
    }

    pub fn mcp_tools_extension_with_servers(servers: &[serde_json::Value]) -> Self {
        Self {
            uri: "systemprompt:mcp-tools".to_string(),
            description: Some("MCP tool execution capabilities with server endpoints".to_string()),
            required: Some(false),
            params: Some(serde_json::json!({
                "supported_protocols": ["mcp-1.0"],
                "servers": servers
            })),
        }
    }

    pub fn opencode_integration_extension() -> Self {
        Self {
            uri: "systemprompt:opencode-integration".to_string(),
            description: Some("OpenCode AI reasoning integration".to_string()),
            required: Some(false),
            params: Some(serde_json::json!({
                "reasoning_model": "claude-3-5-sonnet",
                "execution_mode": "structured_planning"
            })),
        }
    }

    pub fn artifact_rendering_extension() -> Self {
        Self {
            uri: "https://systemprompt.io/extensions/artifact-rendering/v1".to_string(),
            description: Some(
                "MCP tool results rendered as typed artifacts with UI hints".to_string(),
            ),
            required: Some(false),
            params: Some(serde_json::json!({
                "supported_types": ["table", "form", "chart", "tree", "code", "json", "markdown"],
                "version": "1.0.0"
            })),
        }
    }

    pub fn agent_identity(agent_name: &str) -> Self {
        Self {
            uri: "systemprompt:agent-identity".to_string(),
            description: Some("systemprompt.io platform agent name".to_string()),
            required: Some(true),
            params: Some(serde_json::json!({
                "name": agent_name
            })),
        }
    }

    pub fn system_instructions(system_prompt: &str) -> Self {
        Self {
            uri: "systemprompt:system-instructions".to_string(),
            description: Some("Agent system prompt and behavioral guidelines".to_string()),
            required: Some(true),
            params: Some(serde_json::json!({
                "systemPrompt": system_prompt,
                "format": "text/plain"
            })),
        }
    }

    pub fn system_instructions_opt(system_prompt: Option<&str>) -> Option<Self> {
        system_prompt.map(Self::system_instructions)
    }

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
            uri: "systemprompt:service-status".to_string(),
            description: Some("Runtime service status from orchestrator".to_string()),
            required: Some(true),
            params: Some(params),
        }
    }
}
