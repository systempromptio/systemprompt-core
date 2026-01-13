use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::{theme::ColorfulTheme, Input};

use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use super::types::AgentCreateOutput;
use systemprompt_core_logging::CliService;

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long, help = "Agent name")]
    pub name: Option<String>,

    #[arg(long, help = "Port for the agent")]
    pub port: Option<u16>,

    #[arg(long, help = "Display name for the agent")]
    pub display_name: Option<String>,

    #[arg(long, help = "Description of the agent")]
    pub description: Option<String>,

    #[arg(long, help = "Enable the agent after creation")]
    pub enabled: bool,

    #[arg(long, help = "AI provider (e.g., anthropic, openai)")]
    pub provider: Option<String>,

    #[arg(long, help = "AI model (e.g., claude-3-5-sonnet-20241022)")]
    pub model: Option<String>,
}

pub async fn execute(
    args: CreateArgs,
    config: &CliConfig,
) -> Result<CommandResult<AgentCreateOutput>> {
    let name = resolve_input(
        args.name,
        "name",
        config,
        || prompt_name(),
    )?;

    validate_agent_name(&name)?;

    let port = resolve_input(
        args.port,
        "port",
        config,
        || prompt_port(),
    )?;

    let display_name = args.display_name.unwrap_or_else(|| {
        if config.is_interactive() {
            prompt_display_name(&name).unwrap_or_else(|_| name.clone())
        } else {
            name.clone()
        }
    });

    let description = args.description.unwrap_or_else(|| {
        if config.is_interactive() {
            prompt_description().unwrap_or_default()
        } else {
            String::new()
        }
    });

    CliService::info(&format!(
        "Creating agent '{}' on port {} (display: {})...",
        name, port, display_name
    ));

    CliService::warning(
        "Agent creation modifies configuration files. \
         Please add the agent configuration to your services.yaml manually for now."
    );

    let output = AgentCreateOutput {
        name: name.clone(),
        message: format!(
            "Agent '{}' configuration prepared. Add to services.yaml:\n\n\
             agents:\n  \
               {}:\n    \
                 name: {}\n    \
                 port: {}\n    \
                 endpoint: /\n    \
                 enabled: {}\n    \
                 card:\n      \
                   protocolVersion: \"1.0\"\n      \
                   displayName: \"{}\"\n      \
                   description: \"{}\"\n      \
                   version: \"1.0.0\"\n    \
                 metadata:\n      \
                   provider: {}\n      \
                   model: {}",
            name,
            name,
            name,
            port,
            args.enabled,
            display_name,
            description,
            args.provider.as_deref().unwrap_or("anthropic"),
            args.model.as_deref().unwrap_or("claude-3-5-sonnet-20241022")
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
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow!(
            "Agent name must be lowercase alphanumeric with hyphens only"
        ));
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
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err("Name must be lowercase alphanumeric with hyphens only");
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
