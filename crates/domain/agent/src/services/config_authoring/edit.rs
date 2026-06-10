//! Application of [`AgentEditRequest`] mutations to an in-memory
//! [`AgentConfig`].
//!
//! Each `apply_*` function mirrors one group of editable fields and records a
//! human-readable change entry per mutation. Removal targets that are absent
//! from the config are returned to the caller (not logged or printed) so the
//! presentation layer owns how skips are reported.

use std::fs;

use systemprompt_models::services::{AgentConfig, ServicesConfig};

use super::{AgentConfigAuthoringService, ConfigAuthoringError};

#[derive(Debug, Clone, Default)]
pub struct AgentEditRequest {
    pub enable: bool,
    pub disable: bool,
    pub port: Option<u16>,
    pub endpoint: Option<String>,
    pub dev_only: bool,
    pub is_primary: bool,
    pub default: bool,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub icon_url: Option<String>,
    pub documentation_url: Option<String>,
    pub streaming: Option<bool>,
    pub push_notifications: Option<bool>,
    pub state_transition_history: Option<bool>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub system_prompt_file: Option<String>,
    pub mcp_servers: Vec<String>,
    pub remove_mcp_servers: Vec<String>,
    pub skills: Vec<String>,
    pub remove_skills: Vec<String>,
    pub set_values: Vec<String>,
}

impl AgentConfigAuthoringService {
    pub fn apply_enabled_flags(
        agent: &mut AgentConfig,
        request: &AgentEditRequest,
        changes: &mut Vec<String>,
    ) {
        if request.enable {
            agent.enabled = true;
            changes.push("enabled: true".to_owned());
        }
        if request.disable {
            agent.enabled = false;
            changes.push("enabled: false".to_owned());
        }
    }

    pub fn apply_runtime_fields(
        agent: &mut AgentConfig,
        request: &AgentEditRequest,
        changes: &mut Vec<String>,
    ) -> Result<(), ConfigAuthoringError> {
        if let Some(port) = request.port {
            Self::validate_port(port)?;
            agent.port = port;
            changes.push(format!("port: {port}"));
        }
        if let Some(endpoint) = &request.endpoint {
            agent.endpoint.clone_from(endpoint);
            changes.push(format!("endpoint: {endpoint}"));
        }
        if request.dev_only {
            agent.dev_only = true;
            changes.push("dev_only: true".to_owned());
        }
        if request.is_primary {
            agent.is_primary = true;
            changes.push("is_primary: true".to_owned());
        }
        if request.default {
            agent.default = true;
            changes.push("default: true".to_owned());
        }
        Ok(())
    }

    pub fn apply_card_fields(
        agent: &mut AgentConfig,
        request: &AgentEditRequest,
        changes: &mut Vec<String>,
    ) {
        if let Some(display_name) = &request.display_name {
            agent.card.display_name.clone_from(display_name);
            changes.push(format!("card.display_name: {display_name}"));
        }
        if let Some(description) = &request.description {
            agent.card.description.clone_from(description);
            changes.push(format!("card.description: {description}"));
        }
        if let Some(version) = &request.version {
            agent.card.version.clone_from(version);
            changes.push(format!("card.version: {version}"));
        }
        if let Some(icon_url) = &request.icon_url {
            agent.card.icon_url = Some(icon_url.clone());
            changes.push(format!("card.icon_url: {icon_url}"));
        }
        if let Some(documentation_url) = &request.documentation_url {
            agent.card.documentation_url = Some(documentation_url.clone());
            changes.push(format!("card.documentation_url: {documentation_url}"));
        }
    }

    pub fn apply_capability_fields(
        agent: &mut AgentConfig,
        request: &AgentEditRequest,
        changes: &mut Vec<String>,
    ) {
        if let Some(streaming) = request.streaming {
            agent.card.capabilities.streaming = streaming;
            changes.push(format!("card.capabilities.streaming: {streaming}"));
        }
        if let Some(push_notifications) = request.push_notifications {
            agent.card.capabilities.push_notifications = push_notifications;
            changes.push(format!(
                "card.capabilities.push_notifications: {push_notifications}"
            ));
        }
        if let Some(state_transition_history) = request.state_transition_history {
            agent.card.capabilities.state_transition_history = state_transition_history;
            changes.push(format!(
                "card.capabilities.state_transition_history: {state_transition_history}"
            ));
        }
    }

    pub fn apply_metadata_fields(
        agent: &mut AgentConfig,
        request: &AgentEditRequest,
        changes: &mut Vec<String>,
    ) -> Result<(), ConfigAuthoringError> {
        if let Some(provider) = &request.provider {
            agent.metadata.provider = Some(provider.clone());
            changes.push(format!("metadata.provider: {provider}"));
        }
        if let Some(model) = &request.model {
            agent.metadata.model = Some(model.clone());
            changes.push(format!("metadata.model: {model}"));
        }
        if let Some(file_path) = &request.system_prompt_file {
            let content = fs::read_to_string(file_path).map_err(|source| {
                ConfigAuthoringError::SystemPromptFile {
                    path: file_path.clone(),
                    source,
                }
            })?;
            agent.metadata.system_prompt = Some(content.clone());
            changes.push(format!(
                "system_prompt: loaded from {} ({} chars)",
                file_path,
                content.len()
            ));
        } else if let Some(prompt) = &request.system_prompt {
            agent.metadata.system_prompt = Some(prompt.clone());
            changes.push(format!("system_prompt: {} chars", prompt.len()));
        }
        Ok(())
    }

    pub fn apply_mcp_server_changes(
        agent: &mut AgentConfig,
        request: &AgentEditRequest,
        services_config: &ServicesConfig,
        changes: &mut Vec<String>,
    ) -> Result<Vec<String>, ConfigAuthoringError> {
        for mcp_server in &request.mcp_servers {
            if agent.metadata.mcp_servers.include.contains(mcp_server) {
                continue;
            }
            if !services_config.mcp_servers.contains_key(mcp_server) {
                return Err(ConfigAuthoringError::UnknownMcpServer {
                    name: mcp_server.clone(),
                    available: services_config
                        .mcp_servers
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            }
            agent.metadata.mcp_servers.include.push(mcp_server.clone());
            changes.push(format!("added mcp_server: {mcp_server}"));
        }
        let mut skipped = Vec::new();
        for mcp_server in &request.remove_mcp_servers {
            if let Some(pos) = agent
                .metadata
                .mcp_servers
                .include
                .iter()
                .position(|s| s == mcp_server)
            {
                agent.metadata.mcp_servers.include.remove(pos);
                changes.push(format!("removed mcp_server: {mcp_server}"));
            } else {
                skipped.push(mcp_server.clone());
            }
        }
        Ok(skipped)
    }

    pub fn apply_skill_changes(
        agent: &mut AgentConfig,
        request: &AgentEditRequest,
        changes: &mut Vec<String>,
    ) -> Vec<String> {
        for skill in &request.skills {
            if !agent.metadata.skills.include.contains(skill) {
                agent.metadata.skills.include.push(skill.clone());
                changes.push(format!("added skill: {skill}"));
            }
        }
        let mut skipped = Vec::new();
        for skill in &request.remove_skills {
            if let Some(pos) = agent
                .metadata
                .skills
                .include
                .iter()
                .position(|s| s == skill)
            {
                let removed = agent.metadata.skills.include.remove(pos);
                changes.push(format!("removed skill: {removed}"));
            } else {
                skipped.push(skill.clone());
            }
        }
        skipped
    }

    pub fn apply_set_value_changes(
        agent: &mut AgentConfig,
        request: &AgentEditRequest,
        changes: &mut Vec<String>,
    ) -> Result<(), ConfigAuthoringError> {
        for set_value in &request.set_values {
            let Some((key, value)) = set_value.split_once('=') else {
                return Err(ConfigAuthoringError::InvalidSetFormat(set_value.clone()));
            };
            apply_set_value(agent, key, value)?;
            changes.push(format!("{key}: {value}"));
        }
        Ok(())
    }
}

fn apply_set_value(
    agent: &mut AgentConfig,
    key: &str,
    value: &str,
) -> Result<(), ConfigAuthoringError> {
    match key {
        "card.displayName" | "card.display_name" => {
            value.clone_into(&mut agent.card.display_name);
        },
        "card.description" => {
            value.clone_into(&mut agent.card.description);
        },
        "card.version" => {
            value.clone_into(&mut agent.card.version);
        },
        "endpoint" => {
            value.clone_into(&mut agent.endpoint);
        },
        "is_primary" => {
            agent.is_primary = parse_bool(key, value)?;
        },
        "default" => {
            agent.default = parse_bool(key, value)?;
        },
        "dev_only" => {
            agent.dev_only = parse_bool(key, value)?;
        },
        _ => {
            return Err(ConfigAuthoringError::UnknownSetKey(key.to_owned()));
        },
    }
    Ok(())
}

fn parse_bool(key: &str, value: &str) -> Result<bool, ConfigAuthoringError> {
    value
        .parse()
        .map_err(|_e| ConfigAuthoringError::InvalidBoolean {
            key: key.to_owned(),
            value: value.to_owned(),
        })
}
