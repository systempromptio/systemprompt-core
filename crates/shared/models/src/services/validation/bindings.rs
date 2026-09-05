//! Reference resolution and single-owner business rules for services config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::errors::ConfigValidationError;
use crate::services::{ComponentSource, MarketplaceConfig, PluginConfig, ServicesConfig};

impl ServicesConfig {
    // Why: several enabled marketplaces is a supported shape — the manifest is
    // their union — so the selector only has to name a marketplace that exists
    // and is enabled. It no longer decides what ships.
    pub(crate) fn validate_marketplace_selector(&self) -> Result<(), ConfigValidationError> {
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

    pub(crate) fn validate_marketplace_bindings(
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
            if marketplace.enabled && self.mcp_servers.get(mcp_ref).is_some_and(|d| !d.enabled) {
                return Err(ConfigValidationError::business_rule(format!(
                    "Marketplace '{name}': mcp_servers.include names disabled mcp_server \
                     '{mcp_ref}' — enable the server or drop it from the marketplace"
                )));
            }
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

    pub(crate) fn validate_plugin_bindings(
        &self,
        plugin_name: &str,
        plugin: &PluginConfig,
    ) -> Result<(), ConfigValidationError> {
        if plugin.skills.source == ComponentSource::Explicit {
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
            match self.mcp_servers.get(mcp_ref) {
                None => {
                    return Err(ConfigValidationError::unknown_reference(format!(
                        "Plugin '{plugin_name}': mcp_servers.include references unknown \
                         mcp_server '{mcp_ref}'"
                    )));
                },
                Some(deployment) if plugin.enabled && !deployment.enabled => {
                    return Err(ConfigValidationError::business_rule(format!(
                        "Plugin '{plugin_name}' is enabled but depends on disabled mcp_server \
                         '{mcp_ref}' — enable the server or disable the plugin"
                    )));
                },
                Some(_) => {},
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

    pub(crate) fn validate_single_default_agent(&self) -> Result<(), ConfigValidationError> {
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

    pub(crate) fn validate_single_governance_hook_owner(
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
