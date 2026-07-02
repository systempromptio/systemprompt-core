use anyhow::{Context, Result};
use clap::Args;
use std::path::Path;

use super::shared::AgentArgs;
use super::types::AgentCreateOutput;
use crate::CliConfig;
use crate::interactive::{Prompter, resolve_required};
use crate::shared::CommandOutput;
use systemprompt_agent::services::config_authoring::{
    AgentConfigAuthoringService, AgentCreateRequest,
};
use systemprompt_config::ProfileBootstrap;
use systemprompt_loader::ConfigLoader;
use systemprompt_logging::CliService;

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
    system_prompt: String,
}

pub(super) fn execute(
    args: CreateArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let mut agent_args = args.agent;

    let name = resolve_required(args.name, "name", config, || prompt_name(prompter))?;
    AgentConfigAuthoringService::validate_agent_name(&name)?;

    let port = resolve_required(agent_args.port, "port", config, || prompt_port(prompter))?;
    AgentConfigAuthoringService::validate_port(port)?;

    let display_name =
        resolve_display_name(agent_args.display_name.take(), &name, prompter, config);
    let description = resolve_description(agent_args.description.take(), prompter, config);
    let system_prompt = AgentConfigAuthoringService::resolve_system_prompt(
        agent_args.system_prompt_file.as_deref(),
        agent_args.system_prompt.take(),
        &display_name,
        &description,
    )?;

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
    let request = build_create_request(input, args.enabled, agent_args);

    let agent_file = write_agent_config(request)?;

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

fn resolve_display_name(
    arg: Option<String>,
    name: &str,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> String {
    arg.unwrap_or_else(|| {
        if config.is_interactive() {
            prompt_display_name(prompter, name).unwrap_or_else(|_| name.to_owned())
        } else {
            name.to_owned()
        }
    })
}

fn resolve_description(arg: Option<String>, prompter: &dyn Prompter, config: &CliConfig) -> String {
    arg.unwrap_or_else(|| {
        if config.is_interactive() {
            prompt_description(prompter).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to prompt for description");
                String::new()
            })
        } else {
            String::new()
        }
    })
}

fn build_create_request(
    input: ResolvedAgentInput,
    enabled: bool,
    agent: AgentArgs,
) -> AgentCreateRequest {
    AgentCreateRequest {
        name: input.name,
        port: input.port,
        display_name: input.display_name,
        description: input.description,
        system_prompt: input.system_prompt,
        enabled,
        endpoint: agent.endpoint,
        dev_only: agent.dev_only,
        is_primary: agent.is_primary,
        default: agent.default,
        version: agent.version,
        icon_url: agent.icon_url,
        documentation_url: agent.documentation_url,
        streaming: agent.streaming,
        push_notifications: agent.push_notifications,
        state_transition_history: agent.state_transition_history,
        provider: agent.provider,
        model: agent.model,
        mcp_servers: agent.mcp_servers,
        skills: agent.skills,
    }
}

fn write_agent_config(request: AgentCreateRequest) -> Result<std::path::PathBuf> {
    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let services_dir = Path::new(&profile.paths.services);
    let name = request.name.clone();

    let service = AgentConfigAuthoringService::new(services_dir);
    let agent_file = service
        .create(request)
        .with_context(|| format!("Failed to create agent '{}'", name))?;

    ConfigLoader::load().with_context(|| {
        format!(
            "Agent file created at {} but validation failed. Please check the configuration.",
            agent_file.display()
        )
    })?;

    Ok(agent_file)
}

pub fn validate_name_input(input: &str) -> Result<(), &'static str> {
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
}

pub const fn validate_port_input(port: u16) -> Result<(), &'static str> {
    if port == 0 {
        return Err("Port cannot be 0");
    }
    if port < 1024 {
        return Err("Port should be >= 1024 (non-privileged)");
    }
    Ok(())
}

pub fn prompt_name(prompter: &dyn Prompter) -> Result<String> {
    loop {
        let input = prompter
            .input("Agent name")
            .context("Failed to get agent name")?;
        match validate_name_input(&input) {
            Ok(()) => return Ok(input),
            Err(msg) => CliService::warning(msg),
        }
    }
}

pub fn prompt_port(prompter: &dyn Prompter) -> Result<u16> {
    loop {
        let raw = prompter
            .input_with_default("Port", "8001")
            .context("Failed to get port")?;
        let port: u16 = match raw.trim().parse() {
            Ok(port) => port,
            Err(e) => {
                CliService::warning(&format!("'{}' is not a valid port: {}", raw.trim(), e));
                continue;
            },
        };
        match validate_port_input(port) {
            Ok(()) => return Ok(port),
            Err(msg) => CliService::warning(msg),
        }
    }
}

fn prompt_display_name(prompter: &dyn Prompter, default: &str) -> Result<String> {
    prompter
        .input_with_default("Display name", default)
        .context("Failed to get display name")
}

fn prompt_description(prompter: &dyn Prompter) -> Result<String> {
    prompter
        .input_with_default("Description", "")
        .context("Failed to get description")
}
