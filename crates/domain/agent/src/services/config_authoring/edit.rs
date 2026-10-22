//! Application of [`AgentEditRequest`] mutations to an in-memory
//! [`AgentConfig`].
//!
//! Each `apply_*` function mirrors one group of editable fields and records a
//! human-readable change entry per mutation. Removal targets that are absent
//! from the config are returned to the caller (not logged or printed) so the
//! presentation layer owns how skips are reported.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;

use systemprompt_manifest::services::{AgentConfig, ServicesConfig};

use super::{AgentConfigAuthoringService, AgentEditRequest, ConfigAuthoringError};

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
        if let Some(endpoint) = &request.placement.endpoint {
            agent.endpoint.clone_from(endpoint);
            changes.push(format!("endpoint: {endpoint}"));
        }
        if request.placement.dev_only {
            agent.dev_only = true;
            changes.push("dev_only: true".to_owned());
        }
        if request.placement.is_primary {
            agent.is_primary = true;
            changes.push("is_primary: true".to_owned());
        }
        if request.placement.default {
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
        if let Some(display_name) = &request.card.display_name {
            agent.card.display_name.clone_from(display_name);
            changes.push(format!("card.display_name: {display_name}"));
        }
        if let Some(description) = &request.card.description {
            agent.card.description.clone_from(description);
            changes.push(format!("card.description: {description}"));
        }
        if let Some(version) = &request.card.links.version {
            agent.card.version.clone_from(version);
            changes.push(format!("card.version: {version}"));
        }
        if let Some(icon_url) = &request.card.links.icon_url {
            agent.card.icon_url = Some(icon_url.clone());
            changes.push(format!("card.icon_url: {icon_url}"));
        }
        if let Some(documentation_url) = &request.card.links.documentation_url {
            agent.card.documentation_url = Some(documentation_url.clone());
            changes.push(format!("card.documentation_url: {documentation_url}"));
        }
    }

    pub fn apply_capability_fields(
        agent: &mut AgentConfig,
        request: &AgentEditRequest,
        changes: &mut Vec<String>,
    ) {
        if let Some(streaming) = request.capabilities.streaming {
            agent.card.capabilities.streaming = streaming;
            changes.push(format!("card.capabilities.streaming: {streaming}"));
        }
        if let Some(push_notifications) = request.capabilities.push_notifications {
            agent.card.capabilities.push_notifications = push_notifications;
            changes.push(format!(
                "card.capabilities.push_notifications: {push_notifications}"
            ));
        }
        if let Some(extended_agent_card) = request.capabilities.extended_agent_card {
            agent.card.capabilities.extended_agent_card = extended_agent_card;
            changes.push(format!(
                "card.capabilities.extended_agent_card: {extended_agent_card}"
            ));
        }
    }

    pub fn apply_metadata_fields(
        agent: &mut AgentConfig,
        request: &AgentEditRequest,
        changes: &mut Vec<String>,
    ) -> Result<(), ConfigAuthoringError> {
        if let Some(provider) = &request.model.provider {
            agent.metadata.provider = Some(provider.clone());
            changes.push(format!("metadata.provider: {provider}"));
        }
        if let Some(model) = &request.model.model {
            agent.metadata.model = Some(model.clone());
            changes.push(format!("metadata.model: {model}"));
        }
        if let Some(file_path) = &request.system_prompt.file {
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
        } else if let Some(prompt) = &request.system_prompt.inline {
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
        for mcp_server in &request.mcp_servers.add {
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
        for mcp_server in &request.mcp_servers.remove {
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
        for skill in &request.skills.add {
            if !agent.metadata.skills.include.contains(skill) {
                agent.metadata.skills.include.push(skill.clone());
                changes.push(format!("added skill: {skill}"));
            }
        }
        let mut skipped = Vec::new();
        for skill in &request.skills.remove {
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
}
