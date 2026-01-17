use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;
use std::fs;
use std::path::Path;

use super::shared::AgentArgs;
use super::types::AgentCreateOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_core_logging::CliService;
use systemprompt_loader::{ConfigLoader, ConfigWriter};
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, CapabilitiesConfig, OAuthConfig,
};

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long, help = "Agent name")]
    pub name: Option<String>,

    #[arg(long, help = "Enable the agent after creation")]
    pub enabled: bool,

    #[command(flatten)]
    pub agent: AgentArgs,
}

pub fn execute(args: CreateArgs, config: &CliConfig) -> Result<CommandResult<AgentCreateOutput>> {
    let name = resolve_input(args.name, "name", config, prompt_name)?;

    validate_agent_name(&name)?;

    let port = resolve_input(args.agent.port, "port", config, prompt_port)?;

    validate_port(port)?;

    let display_name = args.agent.display_name.unwrap_or_else(|| {
        if config.is_interactive() {
            prompt_display_name(&name).unwrap_or_else(|_| name.clone())
        } else {
            name.clone()
        }
    });

    let description = args.agent.description.unwrap_or_else(|| {
        if config.is_interactive() {
            prompt_description().unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to prompt for description");
                String::new()
            })
        } else {
            String::new()
        }
    });

    let system_prompt = if let Some(file_path) = &args.agent.system_prompt_file {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read system prompt file: {}", file_path))?;
        Some(content)
    } else {
        args.agent.system_prompt.clone()
    };

    CliService::info(&format!(
        "Creating agent '{}' on port {} (display: {})...",
        name, port, display_name
    ));

    let agent_config = AgentConfig {
        name: name.clone(),
        port,
        endpoint: args
            .agent
            .endpoint
            .unwrap_or_else(|| format!("/api/v1/agents/{}", name)),
        enabled: args.enabled,
        dev_only: args.agent.dev_only,
        is_primary: args.agent.is_primary,
        default: args.agent.default,
        card: AgentCardConfig {
            protocol_version: "0.3.0".to_string(),
            name: Some(name.clone()),
            display_name,
            description,
            version: args.agent.version.unwrap_or_else(|| "1.0.0".to_string()),
            preferred_transport: "JSONRPC".to_string(),
            icon_url: args.agent.icon_url,
            documentation_url: args.agent.documentation_url,
            provider: None,
            capabilities: CapabilitiesConfig {
                streaming: args.agent.streaming.unwrap_or(true),
                push_notifications: args.agent.push_notifications.unwrap_or(false),
                state_transition_history: args.agent.state_transition_history.unwrap_or(true),
            },
            default_input_modes: vec!["text/plain".to_string()],
            default_output_modes: vec!["text/plain".to_string()],
            security_schemes: None,
            security: None,
            skills: vec![],
            supports_authenticated_extended_card: false,
        },
        metadata: AgentMetadataConfig {
            system_prompt,
            mcp_servers: args.agent.mcp_servers,
            skills: args.agent.skills,
            provider: Some(
                args.agent
                    .provider
                    .unwrap_or_else(|| "anthropic".to_string()),
            ),
            model: Some(
                args.agent
                    .model
                    .unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string()),
            ),
            ..Default::default()
        },
        oauth: OAuthConfig::default(),
    };

    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let services_dir = Path::new(&profile.paths.services);

    let agent_file = ConfigWriter::create_agent(&agent_config, services_dir)
        .with_context(|| format!("Failed to create agent '{}'", name))?;

    ConfigLoader::load().with_context(|| {
        format!(
            "Agent file created at {} but validation failed. Please check the configuration.",
            agent_file.display()
        )
    })?;

    CliService::success(&format!(
        "Agent '{}' created at {}",
        name,
        agent_file.display()
    ));

    let output = AgentCreateOutput {
        name: name.clone(),
        message: format!(
            "Agent '{}' created successfully at {}",
            name,
            agent_file.display()
        ),
    };

    Ok(CommandResult::text(output).with_title("Agent Created"))
}

fn validate_agent_name(name: &str) -> Result<()> {
    if name.len() < 3 || name.len() > 50 {
        return Err(anyhow!("Agent name must be between 3 and 50 characters"));
    }

    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(anyhow!(
            "Agent name must be lowercase alphanumeric with underscores only"
        ));
    }

    Ok(())
}

fn validate_port(port: u16) -> Result<()> {
    if port == 0 {
        return Err(anyhow!("Port cannot be 0"));
    }
    if port < 1024 {
        return Err(anyhow!("Port must be >= 1024 (non-privileged)"));
    }
    Ok(())
}

fn prompt_name() -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Agent name")
        .validate_with(|input: &String| -> Result<(), &str> {
            if input.len() < 3 {
                return Err("Name must be at least 3 characters");
            }
            if !input
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                return Err("Name must be lowercase alphanumeric with underscores only");
            }
            Ok(())
        })
        .interact_text()
        .context("Failed to get agent name")
}

fn prompt_port() -> Result<u16> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Port")
        .default(8001u16)
        .validate_with(|input: &u16| -> Result<(), &str> {
            if *input == 0 {
                return Err("Port cannot be 0");
            }
            if *input < 1024 {
                return Err("Port should be >= 1024 (non-privileged)");
            }
            Ok(())
        })
        .interact()
        .context("Failed to get port")
}

fn prompt_display_name(default: &str) -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Display name")
        .default(default.to_string())
        .interact_text()
        .context("Failed to get display name")
}

fn prompt_description() -> Result<String> {
    Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Description")
        .allow_empty(true)
        .interact_text()
        .context("Failed to get description")
}
