use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use std::fs;
use std::path::Path;

use super::shared::{apply_set_value, AgentArgs};
use super::types::AgentEditOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_logging::CliService;
use systemprompt_loader::{ConfigLoader, ConfigWriter};
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

#[derive(Debug, Args)]
pub struct EditArgs {
    #[arg(help = "Agent name (required in non-interactive mode)")]
    pub name: Option<String>,

    #[arg(
        long = "set",
        value_name = "KEY=VALUE",
        help = "Set a configuration value (advanced)"
    )]
    pub set_values: Vec<String>,

    #[arg(long, help = "Enable the agent", conflicts_with = "disable")]
    pub enable: bool,

    #[arg(long, help = "Disable the agent", conflicts_with = "enable")]
    pub disable: bool,

    #[arg(long = "remove-mcp-server", help = "Remove an MCP server reference")]
    pub remove_mcp_servers: Vec<String>,

    #[arg(long = "remove-skill", help = "Remove a skill reference")]
    pub remove_skills: Vec<String>,

    #[command(flatten)]
    pub agent: AgentArgs,
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

    if let Some(port) = args.agent.port {
        if port == 0 {
            return Err(anyhow!("Port cannot be 0"));
        }
        if port < 1024 {
            return Err(anyhow!("Port must be >= 1024 (non-privileged)"));
        }
        agent.port = port;
        changes.push(format!("port: {}", port));
    }
    if let Some(endpoint) = &args.agent.endpoint {
        agent.endpoint.clone_from(endpoint);
        changes.push(format!("endpoint: {}", endpoint));
    }
    if args.agent.dev_only {
        agent.dev_only = true;
        changes.push("dev_only: true".to_string());
    }
    if args.agent.is_primary {
        agent.is_primary = true;
        changes.push("is_primary: true".to_string());
    }
    if args.agent.default {
        agent.default = true;
        changes.push("default: true".to_string());
    }

    if let Some(display_name) = &args.agent.display_name {
        agent.card.display_name.clone_from(display_name);
        changes.push(format!("card.display_name: {}", display_name));
    }
    if let Some(description) = &args.agent.description {
        agent.card.description.clone_from(description);
        changes.push(format!("card.description: {}", description));
    }
    if let Some(version) = &args.agent.version {
        agent.card.version.clone_from(version);
        changes.push(format!("card.version: {}", version));
    }
    if let Some(icon_url) = &args.agent.icon_url {
        agent.card.icon_url = Some(icon_url.clone());
        changes.push(format!("card.icon_url: {}", icon_url));
    }
    if let Some(documentation_url) = &args.agent.documentation_url {
        agent.card.documentation_url = Some(documentation_url.clone());
        changes.push(format!("card.documentation_url: {}", documentation_url));
    }
    if let Some(streaming) = args.agent.streaming {
        agent.card.capabilities.streaming = streaming;
        changes.push(format!("card.capabilities.streaming: {}", streaming));
    }
    if let Some(push_notifications) = args.agent.push_notifications {
        agent.card.capabilities.push_notifications = push_notifications;
        changes.push(format!(
            "card.capabilities.push_notifications: {}",
            push_notifications
        ));
    }
    if let Some(state_transition_history) = args.agent.state_transition_history {
        agent.card.capabilities.state_transition_history = state_transition_history;
        changes.push(format!(
            "card.capabilities.state_transition_history: {}",
            state_transition_history
        ));
    }

    if let Some(provider) = &args.agent.provider {
        agent.metadata.provider = Some(provider.clone());
        changes.push(format!("metadata.provider: {}", provider));
    }
    if let Some(model) = &args.agent.model {
        agent.metadata.model = Some(model.clone());
        changes.push(format!("metadata.model: {}", model));
    }

    if let Some(file_path) = &args.agent.system_prompt_file {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read system prompt file: {}", file_path))?;
        agent.metadata.system_prompt = Some(content.clone());
        changes.push(format!(
            "system_prompt: loaded from {} ({} chars)",
            file_path,
            content.len()
        ));
    } else if let Some(prompt) = &args.agent.system_prompt {
        agent.metadata.system_prompt = Some(prompt.clone());
        changes.push(format!("system_prompt: {} chars", prompt.len()));
    }

    for mcp_server in &args.agent.mcp_servers {
        if !agent.metadata.mcp_servers.contains(mcp_server) {
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

    for skill in &args.agent.skills {
        if !agent.metadata.skills.contains(skill) {
            agent.metadata.skills.push(skill.clone());
            changes.push(format!("added skill: {}", skill));
        }
    }
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
            "No changes specified. Use flags like --port, --display-name, --provider, --model, \
             --mcp-server, --skill, --system-prompt, --enable/--disable, etc."
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
