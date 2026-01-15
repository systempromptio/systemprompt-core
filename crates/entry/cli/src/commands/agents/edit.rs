use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use std::fs;
use std::path::Path;

use super::types::AgentEditOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_core_logging::CliService;
use systemprompt_loader::{ConfigLoader, ConfigWriter};
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

#[derive(Debug, Args)]
pub struct EditArgs {
    #[arg(help = "Agent name (required in non-interactive mode)")]
    pub name: Option<String>,

    #[arg(
        long = "set",
        value_name = "KEY=VALUE",
        help = "Set a configuration value"
    )]
    pub set_values: Vec<String>,

    #[arg(long, help = "Enable the agent", conflicts_with = "disable")]
    pub enable: bool,

    #[arg(long, help = "Disable the agent", conflicts_with = "enable")]
    pub disable: bool,

    #[arg(long, help = "Set the port")]
    pub port: Option<u16>,

    #[arg(long, help = "Set the AI provider")]
    pub provider: Option<String>,

    #[arg(long, help = "Set the AI model")]
    pub model: Option<String>,

    #[arg(
        long = "mcp-server",
        help = "Add an MCP server reference (can be specified multiple times)"
    )]
    pub mcp_servers: Vec<String>,

    #[arg(long = "remove-mcp-server", help = "Remove an MCP server reference")]
    pub remove_mcp_servers: Vec<String>,

    #[arg(long, help = "Add a skill reference (can be specified multiple times)")]
    pub skill: Vec<String>,

    #[arg(long = "remove-skill", help = "Remove a skill reference")]
    pub remove_skills: Vec<String>,

    #[arg(long = "system-prompt", help = "Set the system prompt inline")]
    pub system_prompt: Option<String>,

    #[arg(long = "system-prompt-file", help = "Load system prompt from a file")]
    pub system_prompt_file: Option<String>,
}

pub fn execute(args: EditArgs, config: &CliConfig) -> Result<CommandResult<AgentEditOutput>> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let name = resolve_input(args.name, "name", config, || {
        prompt_agent_selection(&services_config)
    })?;

    let mut agent = services_config
        .agents
        .get(&name)
        .ok_or_else(|| anyhow!("Agent '{}' not found", name))?
        .clone();

    let mut changes = Vec::new();

    if args.enable {
        agent.enabled = true;
        changes.push("enabled: true".to_string());
    }

    if args.disable {
        agent.enabled = false;
        changes.push("enabled: false".to_string());
    }

    if let Some(port) = args.port {
        if port == 0 {
            return Err(anyhow!("Port cannot be 0"));
        }
        if port < 1024 {
            return Err(anyhow!("Port must be >= 1024 (non-privileged)"));
        }
        agent.port = port;
        changes.push(format!("port: {}", port));
    }

    if let Some(provider) = &args.provider {
        agent.metadata.provider = Some(provider.clone());
        changes.push(format!("metadata.provider: {}", provider));
    }

    if let Some(model) = &args.model {
        agent.metadata.model = Some(model.clone());
        changes.push(format!("metadata.model: {}", model));
    }

    // Handle MCP server additions
    for mcp_server in &args.mcp_servers {
        if !agent.metadata.mcp_servers.contains(mcp_server) {
            // Validate MCP server exists in config
            if !services_config.mcp_servers.contains_key(mcp_server) {
                return Err(anyhow!(
                    "MCP server '{}' not found in configuration. Available servers: {}",
                    mcp_server,
                    services_config
                        .mcp_servers
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            agent.metadata.mcp_servers.push(mcp_server.clone());
            changes.push(format!("added mcp_server: {}", mcp_server));
        }
    }

    // Handle MCP server removals
    for mcp_server in &args.remove_mcp_servers {
        if let Some(pos) = agent
            .metadata
            .mcp_servers
            .iter()
            .position(|s| s == mcp_server)
        {
            agent.metadata.mcp_servers.remove(pos);
            changes.push(format!("removed mcp_server: {}", mcp_server));
        } else {
            CliService::warning(&format!(
                "MCP server '{}' not found in agent configuration, skipping removal",
                mcp_server
            ));
        }
    }

    // Handle skill additions
    for skill in &args.skill {
        if !agent.metadata.skills.contains(skill) {
            agent.metadata.skills.push(skill.clone());
            changes.push(format!("added skill: {}", skill));
        }
    }

    // Handle skill removals
    for skill in &args.remove_skills {
        if let Some(pos) = agent.metadata.skills.iter().position(|s| s == skill) {
            let removed = agent.metadata.skills.remove(pos);
            changes.push(format!("removed skill: {}", removed));
        } else {
            CliService::warning(&format!(
                "Skill '{}' not found in agent configuration, skipping removal",
                skill
            ));
        }
    }

    // Handle system prompt from file
    if let Some(file_path) = &args.system_prompt_file {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read system prompt file: {}", file_path))?;
        agent.metadata.system_prompt = Some(content.clone());
        changes.push(format!(
            "system_prompt: loaded from {} ({} chars)",
            file_path,
            content.len()
        ));
    }

    // Handle inline system prompt (takes precedence if both specified)
    if let Some(prompt) = &args.system_prompt {
        agent.metadata.system_prompt = Some(prompt.clone());
        changes.push(format!("system_prompt: {} chars", prompt.len()));
    }

    for set_value in &args.set_values {
        let parts: Vec<&str> = set_value.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(anyhow!(
                "Invalid --set format: '{}'. Expected key=value",
                set_value
            ));
        }
        let key = parts[0];
        let value = parts[1];

        apply_set_value(&mut agent, key, value)?;
        changes.push(format!("{}: {}", key, value));
    }

    if changes.is_empty() {
        return Err(anyhow!(
            "No changes specified. Use --enable, --disable, --port, --provider, --model, \
             --mcp-server, --skill, --system-prompt, --system-prompt-file, or --set key=value"
        ));
    }

    CliService::info(&format!("Updating agent '{}'...", name));

    let profile = ProfileBootstrap::get().context("Failed to get profile")?;
    let services_dir = Path::new(&profile.paths.services);

    ConfigWriter::update_agent(&name, &agent, services_dir)
        .with_context(|| format!("Failed to update agent '{}'", name))?;

    ConfigLoader::load().with_context(|| {
        format!(
            "Agent '{}' updated but validation failed. Please check the configuration.",
            name
        )
    })?;

    CliService::success(&format!("Agent '{}' updated successfully", name));

    let output = AgentEditOutput {
        name: name.clone(),
        message: format!(
            "Agent '{}' updated successfully with {} change(s)",
            name,
            changes.len()
        ),
        changes,
    };

    Ok(CommandResult::text(output).with_title(format!("Edit Agent: {}", name)))
}

fn apply_set_value(
    agent: &mut systemprompt_models::services::AgentConfig,
    key: &str,
    value: &str,
) -> Result<()> {
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

fn prompt_agent_selection(config: &systemprompt_models::ServicesConfig) -> Result<String> {
    let mut agents: Vec<&String> = config.agents.keys().collect();
    agents.sort();

    if agents.is_empty() {
        return Err(anyhow!("No agents configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select agent to edit")
        .items(&agents)
        .default(0)
        .interact()
        .context("Failed to get agent selection")?;

    Ok(agents[selection].clone())
}
