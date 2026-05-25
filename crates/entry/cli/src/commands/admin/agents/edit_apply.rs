use anyhow::{Context, Result, anyhow};
use std::fs;
use systemprompt_logging::CliService;

use super::edit::EditArgs;
use super::shared::apply_set_value;

pub(super) fn apply_enabled_flags(
    agent: &mut systemprompt_models::AgentConfig,
    args: &EditArgs,
    changes: &mut Vec<String>,
) {
    if args.enable {
        agent.enabled = true;
        changes.push("enabled: true".to_owned());
    }
    if args.disable {
        agent.enabled = false;
        changes.push("enabled: false".to_owned());
    }
}

pub(super) fn apply_runtime_fields(
    agent: &mut systemprompt_models::AgentConfig,
    args: &EditArgs,
    changes: &mut Vec<String>,
) -> Result<()> {
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
        changes.push("dev_only: true".to_owned());
    }
    if args.agent.is_primary {
        agent.is_primary = true;
        changes.push("is_primary: true".to_owned());
    }
    if args.agent.default {
        agent.default = true;
        changes.push("default: true".to_owned());
    }
    Ok(())
}

pub(super) fn apply_card_fields(
    agent: &mut systemprompt_models::AgentConfig,
    args: &EditArgs,
    changes: &mut Vec<String>,
) {
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
}

pub(super) fn apply_capability_fields(
    agent: &mut systemprompt_models::AgentConfig,
    args: &EditArgs,
    changes: &mut Vec<String>,
) {
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
}

pub(super) fn apply_metadata_fields(
    agent: &mut systemprompt_models::AgentConfig,
    args: &EditArgs,
    changes: &mut Vec<String>,
) -> Result<()> {
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
    Ok(())
}

pub(super) fn apply_mcp_server_changes(
    agent: &mut systemprompt_models::AgentConfig,
    args: &EditArgs,
    services_config: &systemprompt_models::ServicesConfig,
    changes: &mut Vec<String>,
) -> Result<()> {
    for mcp_server in &args.agent.mcp_servers {
        if agent.metadata.mcp_servers.contains(mcp_server) {
            continue;
        }
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
    Ok(())
}

pub(super) fn apply_skill_changes(
    agent: &mut systemprompt_models::AgentConfig,
    args: &EditArgs,
    changes: &mut Vec<String>,
) {
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
}

pub(super) fn apply_set_value_changes(
    agent: &mut systemprompt_models::AgentConfig,
    args: &EditArgs,
    changes: &mut Vec<String>,
) -> Result<()> {
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
        apply_set_value(agent, key, value)?;
        changes.push(format!("{}: {}", key, value));
    }
    Ok(())
}
