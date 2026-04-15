use super::super::ai::ToolModelOverrides;
use super::super::auth::{JwtAudience, Permission};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const AGENT_CONFIG_FILENAME: &str = "config.yaml";
pub const DEFAULT_AGENT_SYSTEM_PROMPT_FILE: &str = "system_prompt.md";

fn default_version() -> String {
    "1.0.0".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiskAgentConfig {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub port: u16,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub dev_only: bool,
    #[serde(default)]
    pub is_primary: bool,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub system_prompt_file: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    pub card: AgentCardConfig,
    #[serde(default)]
    pub oauth: OAuthConfig,
}

impl DiskAgentConfig {
    pub fn system_prompt_file(&self) -> &str {
        self.system_prompt_file
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_AGENT_SYSTEM_PROMPT_FILE)
    }

    pub fn to_agent_config(&self, base_url: &str, system_prompt: Option<String>) -> AgentConfig {
        let endpoint = self.endpoint.clone().unwrap_or_else(|| {
            format!(
                "{}/api/v1/agents/{}",
                base_url.trim_end_matches('/'),
                self.name
            )
        });

        let card_name = self
            .card
            .name
            .clone()
            .unwrap_or_else(|| self.display_name.clone());

        AgentConfig {
            name: self.name.clone(),
            port: self.port,
            endpoint,
            enabled: self.enabled,
            dev_only: self.dev_only,
            is_primary: self.is_primary,
            default: self.default,
            card: AgentCardConfig {
                name: Some(card_name),
                ..self.card.clone()
            },
            metadata: AgentMetadataConfig {
                system_prompt,
                mcp_servers: self.mcp_servers.clone(),
                skills: self.skills.clone(),
                provider: self.provider.clone(),
                model: self.model.clone(),
                ..Default::default()
            },
            oauth: self.oauth.clone(),
        }
    }

    pub fn validate(&self, dir_name: &str) -> anyhow::Result<()> {
        if !self.id.is_empty() && self.id != dir_name {
            anyhow::bail!(
                "Agent config id '{}' does not match directory name '{}'",
                self.id,
                dir_name
            );
        }

        if !self
            .name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            anyhow::bail!(
                "Agent name '{}' must be lowercase alphanumeric with underscores only",
                self.name
            );
        }

        if self.name.len() < 3 || self.name.len() > 50 {
            anyhow::bail!(
                "Agent name '{}' must be between 3 and 50 characters",
                self.name
            );
        }

        if self.port == 0 {
            anyhow::bail!("Agent '{}' has invalid port {}", self.name, self.port);
        }

        if self.display_name.is_empty() {
            anyhow::bail!("Agent '{}' display_name must not be empty", self.name);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub port: u16,
    pub endpoint: String,
    pub enabled: bool,
    #[serde(default)]
    pub dev_only: bool,
    #[serde(default)]
    pub is_primary: bool,
    #[serde(default)]
    pub default: bool,
    pub card: AgentCardConfig,
    pub metadata: AgentMetadataConfig,
    #[serde(default)]
    pub oauth: OAuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCardConfig {
    pub protocol_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub display_name: String,
    pub description: String,
    pub version: String,
    #[serde(default = "default_transport")]
    pub preferred_transport: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<AgentProviderInfo>,
    #[serde(default)]
    pub capabilities: CapabilitiesConfig,
    #[serde(default = "default_input_modes")]
    pub default_input_modes: Vec<String>,
    #[serde(default = "default_output_modes")]
    pub default_output_modes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_schemes: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub skills: Vec<AgentSkillConfig>,
    #[serde(default)]
    pub supports_authenticated_extended_card: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkillConfig {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_modes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_modes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProviderInfo {
    pub organization: String,
    pub url: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitiesConfig {
    #[serde(default = "default_true")]
    pub streaming: bool,
    #[serde(default)]
    pub push_notifications: bool,
    #[serde(default = "default_true")]
    pub state_transition_history: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct AgentMetadataConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(default)]
    pub tool_model_overrides: ToolModelOverrides,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub scopes: Vec<Permission>,
    #[serde(default = "default_audience")]
    pub audience: JwtAudience,
}

impl AgentConfig {
    pub fn validate(&self, name: &str) -> anyhow::Result<()> {
        if self.name != name {
            anyhow::bail!(
                "Agent config key '{}' does not match name field '{}'",
                name,
                self.name
            );
        }

        if !self
            .name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            anyhow::bail!(
                "Agent name '{}' must be lowercase alphanumeric with underscores only",
                self.name
            );
        }

        if self.name.len() < 3 || self.name.len() > 50 {
            anyhow::bail!(
                "Agent name '{}' must be between 3 and 50 characters",
                self.name
            );
        }

        if self.port == 0 {
            anyhow::bail!("Agent '{}' has invalid port {}", self.name, self.port);
        }

        Ok(())
    }

    pub fn extract_oauth_scopes_from_card(&mut self) {
        if let Some(security_vec) = &self.card.security {
            for security_obj in security_vec {
                if let Some(oauth2_scopes) = security_obj.get("oauth2").and_then(|v| v.as_array()) {
                    let mut permissions = Vec::new();
                    for scope_val in oauth2_scopes {
                        if let Some(scope_str) = scope_val.as_str() {
                            match scope_str {
                                "admin" => permissions.push(Permission::Admin),
                                "user" => permissions.push(Permission::User),
                                "service" => permissions.push(Permission::Service),
                                "a2a" => permissions.push(Permission::A2a),
                                "mcp" => permissions.push(Permission::Mcp),
                                "anonymous" => permissions.push(Permission::Anonymous),
                                _ => {},
                            }
                        }
                    }
                    if !permissions.is_empty() {
                        self.oauth.scopes = permissions;
                        self.oauth.required = true;
                    }
                }
            }
        }
    }

    pub fn construct_url(&self, base_url: &str) -> String {
        format!(
            "{}/api/v1/agents/{}",
            base_url.trim_end_matches('/'),
            self.name
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentSummary {
    pub agent_id: String,
    pub name: String,
    pub display_name: String,
    pub port: u16,
    pub enabled: bool,
    pub is_primary: bool,
    pub is_default: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl AgentSummary {
    pub fn from_config(name: &str, config: &AgentConfig) -> Self {
        Self {
            agent_id: name.to_string(),
            name: name.to_string(),
            display_name: config.card.display_name.clone(),
            port: config.port,
            enabled: config.enabled,
            is_primary: config.is_primary,
            is_default: config.default,
            tags: Vec::new(),
        }
    }
}

impl From<&AgentConfig> for AgentSummary {
    fn from(config: &AgentConfig) -> Self {
        Self {
            agent_id: config.name.clone(),
            name: config.name.clone(),
            display_name: config.card.display_name.clone(),
            port: config.port,
            enabled: config.enabled,
            is_primary: config.is_primary,
            is_default: config.default,
            tags: Vec::new(),
        }
    }
}

impl Default for CapabilitiesConfig {
    fn default() -> Self {
        Self {
            streaming: true,
            push_notifications: false,
            state_transition_history: true,
        }
    }
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            required: false,
            scopes: Vec::new(),
            audience: JwtAudience::A2a,
        }
    }
}

fn default_transport() -> String {
    "JSONRPC".to_string()
}

fn default_input_modes() -> Vec<String> {
    vec!["text/plain".to_string()]
}

fn default_output_modes() -> Vec<String> {
    vec!["text/plain".to_string()]
}

const fn default_true() -> bool {
    true
}

const fn default_audience() -> JwtAudience {
    JwtAudience::A2a
}
