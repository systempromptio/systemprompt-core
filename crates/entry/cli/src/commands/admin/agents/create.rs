use anyhow::{Context, Result, anyhow};
use clap::Args;
use dialoguer::Input;
use dialoguer::theme::ColorfulTheme;
use std::fs;
use std::path::Path;

use super::shared::AgentArgs;
use super::types::AgentCreateOutput;
use crate::CliConfig;
use crate::interactive::resolve_required;
use crate::shared::CommandOutput;
use systemprompt_config::ProfileBootstrap;
use systemprompt_loader::{ConfigLoader, ConfigWriter};
use systemprompt_logging::CliService;
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::profile::ProviderRegistry;
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

struct ResolvedAgentInput {
    name: String,
    port: u16,
    display_name: String,
    description: String,
    system_prompt: Option<String>,
}

pub(super) fn execute(args: CreateArgs, config: &CliConfig) -> Result<CommandOutput> {
    let mut agent_args = args.agent;

    let name = resolve_required(args.name, "name", config, prompt_name)?;
    validate_agent_name(&name)?;

    let port = resolve_required(agent_args.port, "port", config, prompt_port)?;
    validate_port(port)?;

    let display_name = resolve_display_name(agent_args.display_name.take(), &name, config);
    let description = resolve_description(agent_args.description.take(), config);
    let system_prompt = resolve_system_prompt(&mut agent_args, &display_name, &description)?;

    CliService::info(&format!(
        "Creating agent '{}' on port {} (display: {})...",
        name, port, display_name
    ));

    let input = ResolvedAgentInput {
        name: name.clone(),
        port,
        display_name,
        description,
        system_prompt,
    };
    let agent_config = build_agent_config(input, args.enabled, agent_args);

    let agent_file = write_agent_config(&agent_config)?;

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

    Ok(CommandOutput::card_value("Agent Created", &output))
}

fn resolve_display_name(arg: Option<String>, name: &str, config: &CliConfig) -> String {
    arg.unwrap_or_else(|| {
        if config.is_interactive() {
            prompt_display_name(name).unwrap_or_else(|_| name.to_owned())
        } else {
            name.to_owned()
        }
    })
}

fn resolve_description(arg: Option<String>, config: &CliConfig) -> String {
    arg.unwrap_or_else(|| {
        if config.is_interactive() {
            prompt_description().unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to prompt for description");
                String::new()
            })
        } else {
            String::new()
        }
    })
}

fn resolve_system_prompt(
    agent: &mut AgentArgs,
    display_name: &str,
    description: &str,
) -> Result<Option<String>> {
    if let Some(file_path) = &agent.system_prompt_file {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read system prompt file: {}", file_path))?;
        return Ok(Some(content));
    }
    if let Some(prompt) = agent.system_prompt.take() {
        return Ok(Some(prompt));
    }
    let default_prompt = if description.is_empty() {
        format!("You are {}.", display_name)
    } else {
        format!("You are {}. {}", display_name, description)
    };
    Ok(Some(default_prompt))
}

fn build_agent_config(input: ResolvedAgentInput, enabled: bool, agent: AgentArgs) -> AgentConfig {
    let provider = agent.provider.unwrap_or_else(|| "anthropic".to_owned());
    let model = agent.model.unwrap_or_else(|| default_model_for(&provider));

    AgentConfig {
        name: input.name.clone(),
        port: input.port,
        endpoint: agent.endpoint.unwrap_or_else(|| {
            ApiPaths::agent_endpoint(&systemprompt_identifiers::AgentId::new(&input.name))
        }),
        enabled,
        dev_only: agent.dev_only,
        is_primary: agent.is_primary,
        default: agent.default,
        tags: Vec::new(),
        card: AgentCardConfig {
            protocol_version: systemprompt_agent::A2A_PROTOCOL_VERSION.to_owned(),
            name: Some(input.name),
            display_name: input.display_name,
            description: input.description,
            version: agent.version.unwrap_or_else(|| "1.0.0".to_owned()),
            preferred_transport: "JSONRPC".to_owned(),
            icon_url: agent.icon_url,
            documentation_url: agent.documentation_url,
            provider: None,
            capabilities: CapabilitiesConfig {
                streaming: agent.streaming.unwrap_or(true),
                push_notifications: agent.push_notifications.unwrap_or(false),
                state_transition_history: agent.state_transition_history.unwrap_or(true),
            },
            default_input_modes: vec!["text/plain".to_owned()],
            default_output_modes: vec!["text/plain".to_owned()],
            security_schemes: None,
            security: None,
            skills: vec![],
            supports_authenticated_extended_card: false,
        },
        metadata: AgentMetadataConfig {
            system_prompt: input.system_prompt,
            mcp_servers: systemprompt_models::services::PluginComponentRef {
                include: agent.mcp_servers,
                ..Default::default()
            },
            skills: systemprompt_models::services::PluginComponentRef {
                include: agent.skills,
                ..Default::default()
            },
            provider: Some(provider),
            model: Some(model),
            ..Default::default()
        },
        oauth: OAuthConfig::default(),
    }
}

fn write_agent_config(agent_config: &AgentConfig) -> Result<std::path::PathBuf> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let services_dir = Path::new(&profile.paths.services);

    let agent_file = ConfigWriter::create_agent(agent_config, services_dir)
        .with_context(|| format!("Failed to create agent '{}'", agent_config.name))?;

    ConfigLoader::load().with_context(|| {
        format!(
            "Agent file created at {} but validation failed. Please check the configuration.",
            agent_file.display()
        )
    })?;

    Ok(agent_file)
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
        .default(default.to_owned())
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

// Why: the seed catalog is the single source of valid out-of-box model ids;
// deriving the provider's default here keeps agent-create from pinning a
// retired id that would 404 on first inference.
fn default_model_for(provider: &str) -> String {
    ProviderRegistry::default_seed()
        .ok()
        .and_then(|registry| {
            registry
                .find_provider(provider)
                .and_then(|entry| entry.models.first().map(|m| m.id.as_str().to_owned()))
        })
        .unwrap_or_else(|| "claude-sonnet-4-6".to_owned())
}
