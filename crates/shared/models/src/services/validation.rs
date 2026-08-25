//! Cross-reference and port validation for [`ServicesConfig`].
//!
//! These private methods back [`ServicesConfig::validate`]: port conflict and
//! range checks, marketplace/plugin/skill `include` reference resolution, and
//! the single-default-agent and default-marketplace-selector business rules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use crate::errors::ConfigValidationError;
use crate::mcp::McpServerType;

use super::{MarketplaceConfig, PluginConfig, ServicesConfig};

impl ServicesConfig {
    pub(super) fn validate_ports(&self) -> Result<(), ConfigValidationError> {
        self.validate_port_conflicts()?;
        self.validate_port_ranges()?;
        self.validate_mcp_port_ranges()
    }

    pub(super) fn validate_default_marketplace_selector(
        &self,
    ) -> Result<(), ConfigValidationError> {
        let enabled_count = self.marketplaces.values().filter(|m| m.enabled).count();
        if enabled_count > 1 && self.settings.default_marketplace_id.is_none() {
            return Err(ConfigValidationError::business_rule(format!(
                "{enabled_count} enabled marketplaces are configured but \
                 settings.default_marketplace_id is unset; set it to select the active \
                 marketplace"
            )));
        }

        if let Some(id) = &self.settings.default_marketplace_id {
            let Some(marketplace) = self
                .marketplaces
                .iter()
                .find(|(k, _)| k.as_str() == id.as_str())
                .map(|(_, m)| m)
            else {
                return Err(ConfigValidationError::unknown_reference(format!(
                    "settings.default_marketplace_id '{}' does not match any configured \
                     marketplace",
                    id.as_str()
                )));
            };
            if !marketplace.enabled {
                return Err(ConfigValidationError::business_rule(format!(
                    "settings.default_marketplace_id '{}' selects a disabled marketplace",
                    id.as_str()
                )));
            }
        }

        Ok(())
    }

    pub(super) fn validate_marketplace_bindings(
        &self,
        name: &str,
        marketplace: &MarketplaceConfig,
    ) -> Result<(), ConfigValidationError> {
        for plugin_ref in &marketplace.plugins.include {
            if !self.plugins.contains_key(plugin_ref) {
                return Err(ConfigValidationError::unknown_reference(format!(
                    "Marketplace '{name}': plugins.include references unknown plugin \
                     '{plugin_ref}'"
                )));
            }
        }

        for plugin_ref in &marketplace.plugins.include {
            if self.plugins.get(plugin_ref).is_some_and(|p| !p.enabled) {
                tracing::warn!(
                    marketplace = name,
                    plugin = plugin_ref,
                    "Marketplace includes a disabled plugin; it will ship nothing"
                );
            }
        }

        for mcp_ref in &marketplace.mcp_servers.include {
            if !self.mcp_servers.contains_key(mcp_ref) {
                return Err(ConfigValidationError::unknown_reference(format!(
                    "Marketplace '{name}': mcp_servers.include references unknown mcp_server \
                     '{mcp_ref}'"
                )));
            }
        }

        for agent_ref in &marketplace.agents.include {
            if !self.agents.contains_key(agent_ref) {
                return Err(ConfigValidationError::unknown_reference(format!(
                    "Marketplace '{name}': agents.include references unknown agent '{agent_ref}'"
                )));
            }
        }

        Ok(())
    }

    pub(super) fn validate_plugin_bindings(
        &self,
        plugin_name: &str,
        plugin: &PluginConfig,
    ) -> Result<(), ConfigValidationError> {
        if plugin.skills.source == super::ComponentSource::Explicit {
            for skill_ref in &plugin.skills.include {
                if !self.skills.skills.contains_key(skill_ref) {
                    return Err(ConfigValidationError::unknown_reference(format!(
                        "Plugin '{plugin_name}': skills.include references unknown skill \
                         '{skill_ref}'"
                    )));
                }
            }
        }

        for skill_ref in &plugin.skills.exclude {
            if !self.skills.skills.contains_key(skill_ref) {
                tracing::warn!(
                    plugin = plugin_name,
                    skill = skill_ref,
                    "Plugin skills.exclude references unknown skill; the entry excludes nothing"
                );
            }
        }

        for agent_ref in &plugin.agents.exclude {
            if !self.agents.contains_key(agent_ref) {
                tracing::warn!(
                    plugin = plugin_name,
                    agent = agent_ref,
                    "Plugin agents.exclude references unknown agent; the entry excludes nothing"
                );
            }
        }

        for mcp_ref in &plugin.mcp_servers.include {
            if !self.mcp_servers.contains_key(mcp_ref) {
                return Err(ConfigValidationError::unknown_reference(format!(
                    "Plugin '{plugin_name}': mcp_servers.include references unknown mcp_server \
                     '{mcp_ref}'"
                )));
            }
        }

        for agent_ref in &plugin.agents.include {
            if !self.agents.contains_key(agent_ref) {
                return Err(ConfigValidationError::unknown_reference(format!(
                    "Plugin '{plugin_name}': agents.include references unknown agent '{agent_ref}'"
                )));
            }
        }

        Ok(())
    }

    pub(super) fn validate_skills(&self) -> Result<(), ConfigValidationError> {
        for (key, skill) in &self.skills.skills {
            if !skill.id.as_str().is_empty() && skill.id.as_str() != key.as_str() {
                return Err(ConfigValidationError::invalid_field(format!(
                    "Skill map key '{}' does not match skill id '{}'",
                    key, skill.id
                )));
            }

            let id = key.as_str();
            if id.len() < 3 || id.len() > 64 {
                return Err(ConfigValidationError::invalid_field(format!(
                    "Skill '{key}': id must be between 3 and 64 characters"
                )));
            }
            if !id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                return Err(ConfigValidationError::invalid_field(format!(
                    "Skill '{key}': id must be lowercase alphanumeric with underscores only \
                     (snake_case)"
                )));
            }

            for agent_ref in &skill.assigned_agents.include {
                if !self.agents.contains_key(agent_ref) {
                    tracing::warn!(
                        skill = %key,
                        agent = %agent_ref,
                        "Skill references agent that is not defined in services config"
                    );
                }
            }

            for mcp_ref in &skill.mcp_servers.include {
                if !self.mcp_servers.contains_key(mcp_ref) {
                    tracing::warn!(
                        skill = %key,
                        mcp_server = %mcp_ref,
                        "Skill references MCP server that is not defined in services config"
                    );
                }
            }
        }

        Ok(())
    }

    fn validate_port_conflicts(&self) -> Result<(), ConfigValidationError> {
        let mut seen_ports = HashMap::new();

        for (name, agent) in &self.agents {
            if let Some(existing) = seen_ports.insert(agent.port, ("agent", name.as_str())) {
                return Err(ConfigValidationError::port_conflict(format!(
                    "Port conflict: {} used by both {} '{}' and agent '{}'",
                    agent.port, existing.0, existing.1, name
                )));
            }
        }

        for (name, mcp) in &self.mcp_servers {
            if mcp.server_type == McpServerType::External {
                continue;
            }
            if let Some(existing) = seen_ports.insert(mcp.port, ("mcp_server", name.as_str())) {
                return Err(ConfigValidationError::port_conflict(format!(
                    "Port conflict: {} used by both {} '{}' and mcp_server '{}'",
                    mcp.port, existing.0, existing.1, name
                )));
            }
        }

        Ok(())
    }

    fn validate_port_ranges(&self) -> Result<(), ConfigValidationError> {
        let (min, max) = self.settings.agent_port_range;

        for (name, agent) in &self.agents {
            if agent.port < min || agent.port > max {
                return Err(ConfigValidationError::invalid_field(format!(
                    "Agent '{}' port {} is outside allowed range {}-{}",
                    name, agent.port, min, max
                )));
            }
        }

        Ok(())
    }

    fn validate_mcp_port_ranges(&self) -> Result<(), ConfigValidationError> {
        let (min, max) = self.settings.mcp_port_range;

        for (name, mcp) in &self.mcp_servers {
            if mcp.server_type == McpServerType::External {
                continue;
            }
            if mcp.port < min || mcp.port > max {
                return Err(ConfigValidationError::invalid_field(format!(
                    "MCP server '{}' port {} is outside allowed range {}-{}",
                    name, mcp.port, min, max
                )));
            }
        }

        Ok(())
    }

    pub(super) fn validate_single_default_agent(&self) -> Result<(), ConfigValidationError> {
        let default_agents: Vec<&str> = self
            .agents
            .iter()
            .filter_map(|(name, agent)| {
                if agent.default {
                    Some(name.as_str())
                } else {
                    None
                }
            })
            .collect();

        match default_agents.len() {
            0 | 1 => Ok(()),
            _ => Err(ConfigValidationError::business_rule(format!(
                "Multiple agents marked as default: {}. Only one agent can have 'default: true'",
                default_agents.join(", ")
            ))),
        }
    }

    pub(super) fn validate_single_governance_hook_owner(
        &self,
    ) -> Result<(), ConfigValidationError> {
        let owners: Vec<&str> = self
            .plugins
            .values()
            .filter(|p| p.enabled && p.hooks.governance)
            .map(|p| p.id.as_str())
            .collect();

        match owners.len() {
            0 => {
                tracing::warn!(
                    "no enabled plugin sets 'hooks.governance: true' — governance hooks will \
                     not be installed, so no tool call will be checked"
                );
                Ok(())
            },
            1 => Ok(()),
            _ => Err(ConfigValidationError::business_rule(format!(
                "Multiple plugins set 'hooks.governance: true': {}. Hooks run session-globally, \
                 so at most one plugin may own them",
                owners.join(", ")
            ))),
        }
    }
}
