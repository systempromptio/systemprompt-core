use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_models::services::AgentConfig;

#[derive(Debug, Args, Default, Clone)]
pub struct AgentArgs {
    #[arg(long, help = "Port for the agent")]
    pub port: Option<u16>,

    #[arg(long, help = "Custom endpoint path (default: /api/v1/agents/{name})")]
    pub endpoint: Option<String>,

    #[arg(long, help = "Mark agent as dev-only (won't run in production)")]
    pub dev_only: bool,

    #[arg(long, help = "Mark agent as primary")]
    pub is_primary: bool,

    #[arg(long, help = "Mark agent as the default agent")]
    pub default: bool,

    #[arg(long, help = "Display name for the agent")]
    pub display_name: Option<String>,

    #[arg(long, help = "Description of the agent")]
    pub description: Option<String>,

    #[arg(long, help = "Version string for the agent (default: 1.0.0)")]
    pub version: Option<String>,

    #[arg(long, help = "URL to the agent's icon")]
    pub icon_url: Option<String>,

    #[arg(long, help = "URL to the agent's documentation")]
    pub documentation_url: Option<String>,

    #[arg(long, help = "Enable streaming capability (default: true)")]
    pub streaming: Option<bool>,

    #[arg(long, help = "Enable push notifications capability")]
    pub push_notifications: Option<bool>,

    #[arg(long, help = "Enable state transition history (default: true)")]
    pub state_transition_history: Option<bool>,

    #[arg(long, help = "AI provider (e.g., anthropic, openai, gemini)")]
    pub provider: Option<String>,

    #[arg(long, help = "AI model (e.g., claude-3-5-sonnet-20241022)")]
    pub model: Option<String>,

    #[arg(long = "system-prompt", help = "Set the system prompt inline")]
    pub system_prompt: Option<String>,

    #[arg(long = "system-prompt-file", help = "Load system prompt from a file")]
    pub system_prompt_file: Option<String>,

    #[arg(
        long = "mcp-server",
        help = "Add an MCP server reference (can be specified multiple times)"
    )]
    pub mcp_servers: Vec<String>,

    #[arg(
        long = "skill",
        help = "Add a skill reference (can be specified multiple times)"
    )]
    pub skills: Vec<String>,
}

impl AgentArgs {
    pub fn has_any_value(&self) -> bool {
        self.port.is_some()
            || self.endpoint.is_some()
            || self.dev_only
            || self.is_primary
            || self.default
            || self.display_name.is_some()
            || self.description.is_some()
            || self.version.is_some()
            || self.icon_url.is_some()
            || self.documentation_url.is_some()
            || self.streaming.is_some()
            || self.push_notifications.is_some()
            || self.state_transition_history.is_some()
            || self.provider.is_some()
            || self.model.is_some()
            || self.system_prompt.is_some()
            || self.system_prompt_file.is_some()
            || !self.mcp_servers.is_empty()
            || !self.skills.is_empty()
    }
}

pub fn apply_set_value(agent: &mut AgentConfig, key: &str, value: &str) -> Result<()> {
    match key {
        "card.displayName" | "card.display_name" => {
            agent.card.display_name = value.to_string();
        },
        "card.description" => {
            agent.card.description = value.to_string();
        },
        "card.version" => {
            agent.card.version = value.to_string();
        },
        "endpoint" => {
            agent.endpoint = value.to_string();
        },
        "is_primary" => {
            agent.is_primary = value
                .parse()
                .map_err(|_| anyhow!("Invalid boolean value for is_primary: '{}'", value))?;
        },
        "default" => {
            agent.default = value
                .parse()
                .map_err(|_| anyhow!("Invalid boolean value for default: '{}'", value))?;
        },
        "dev_only" => {
            agent.dev_only = value
                .parse()
                .map_err(|_| anyhow!("Invalid boolean value for dev_only: '{}'", value))?;
        },
        _ => {
            return Err(anyhow!(
                "Unknown configuration key: '{}'. Supported keys: card.displayName, \
                 card.description, card.version, endpoint, is_primary, default, dev_only",
                key
            ));
        },
    }
    Ok(())
}
