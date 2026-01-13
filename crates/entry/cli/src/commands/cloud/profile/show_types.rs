use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use systemprompt_models::{
    AgentConfig, AiConfig, ContentConfigRaw, Deployment, SkillsConfig, WebConfig,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct FullConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<EnvironmentConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<SettingsOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<HashMap<String, AgentConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, Deployment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<SkillsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai: Option<AiConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web: Option<WebConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<ContentConfigRaw>,
}

impl FullConfig {
    pub const fn empty() -> Self {
        Self {
            environment: None,
            settings: None,
            agents: None,
            mcp_servers: None,
            skills: None,
            ai: None,
            web: None,
            content: None,
        }
    }

    pub fn with_environment(mut self, environment: EnvironmentConfig) -> Self {
        self.environment = Some(environment);
        self
    }

    pub fn with_settings(mut self, settings: SettingsOutput) -> Self {
        self.settings = Some(settings);
        self
    }

    pub fn with_agents(mut self, agents: HashMap<String, AgentConfig>) -> Self {
        self.agents = Some(agents);
        self
    }

    pub fn with_mcp_servers(mut self, mcp_servers: HashMap<String, Deployment>) -> Self {
        self.mcp_servers = Some(mcp_servers);
        self
    }

    pub fn with_skills(mut self, skills: SkillsConfig) -> Self {
        self.skills = Some(skills);
        self
    }

    pub fn with_ai(mut self, ai: AiConfig) -> Self {
        self.ai = Some(ai);
        self
    }

    pub fn with_web(mut self, web: WebConfig) -> Self {
        self.web = Some(web);
        self
    }

    pub fn with_content(mut self, content: ContentConfigRaw) -> Self {
        self.content = Some(content);
        self
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    pub core: CoreEnvVars,
    pub systemprompt: SystemPromptEnvVars,
    pub database: DatabaseEnvVars,
    pub jwt: JwtEnvVars,
    pub rate_limits: RateLimitEnvVars,
    pub paths: PathsEnvVars,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoreEnvVars {
    pub sitename: String,
    pub host: String,
    pub port: u16,
    pub api_server_url: String,
    pub api_external_url: String,
    pub use_https: bool,
    pub github_link: String,
    pub github_token: Option<String>,
    pub cors_allowed_origins: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemPromptEnvVars {
    pub env: String,
    pub verbosity: String,
    pub services_path: Option<String>,
    pub skills_path: Option<String>,
    pub config_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseEnvVars {
    pub database_type: String,
    pub database_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtEnvVars {
    pub issuer: String,
    pub secret: String,
    pub access_token_expiration: i64,
    pub refresh_token_expiration: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitEnvVars {
    pub disabled: bool,
    pub burst_multiplier: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PathsEnvVars {
    pub system_path: String,
    pub services: String,
    pub skills: String,
    pub services_config: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsOutput {
    pub agent_port_range: (u16, u16),
    pub mcp_port_range: (u16, u16),
    pub auto_start_enabled: bool,
    pub validation_strict: bool,
    pub schema_validation_mode: String,
}
