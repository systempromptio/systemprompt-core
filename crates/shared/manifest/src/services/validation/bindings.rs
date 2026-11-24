//! Reference resolution for services config bindings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::{MarketplaceConfig, PluginConfig, ServicesConfig};
use systemprompt_models::errors::{ServicesValidationError, ValidationSubject};
use systemprompt_models::plugin::ComponentSource;

impl ServicesConfig {
    pub(crate) fn validate_marketplace_selector(&self) -> Result<(), ServicesValidationError> {
        if let Some(id) = &self.settings.default_marketplace_id {
            let Some(marketplace) = self
                .marketplaces
                .iter()
                .find(|(k, _)| k.as_str() == id.as_str())
                .map(|(_, m)| m)
            else {
                return Err(ServicesValidationError::unknown_reference(
                    ValidationSubject::clause("settings.default_marketplace_id", id.as_str()),
                    "does not match any configured \
                     marketplace",
                ));
            };
            if !marketplace.enabled {
                return Err(ServicesValidationError::business_rule(
                    ValidationSubject::clause("settings.default_marketplace_id", id.as_str()),
                    "selects a disabled marketplace",
                ));
            }
        }

        Ok(())
    }

    pub(crate) fn validate_marketplace_bindings(
        &self,
        name: &str,
        marketplace: &MarketplaceConfig,
    ) -> Result<(), ServicesValidationError> {
        for plugin_ref in &marketplace.plugins.include {
            if !self.plugins.contains_key(plugin_ref) {
                return Err(ServicesValidationError::unknown_reference(
                    ValidationSubject::named("Marketplace", name),
                    format!(
                        "plugins.include references unknown plugin \
                     '{plugin_ref}'"
                    ),
                ));
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
                return Err(ServicesValidationError::business_rule(
                    ValidationSubject::named("Marketplace", name),
                    format!(
                        "mcp_servers.include names disabled mcp_server \
                     '{mcp_ref}' — enable the server or drop it from the marketplace"
                    ),
                ));
            }
            if !self.mcp_servers.contains_key(mcp_ref) {
                return Err(ServicesValidationError::unknown_reference(
                    ValidationSubject::named("Marketplace", name),
                    format!(
                        "mcp_servers.include references unknown mcp_server \
                     '{mcp_ref}'"
                    ),
                ));
            }
        }

        for agent_ref in &marketplace.agents.include {
            if !self.agents.contains_key(agent_ref) {
                return Err(ServicesValidationError::unknown_reference(
                    ValidationSubject::named("Marketplace", name),
                    format!("agents.include references unknown agent '{agent_ref}'"),
                ));
            }
        }

        Ok(())
    }

    // Why: Claude Code refuses a cross-marketplace dependency unless the
    // carrying marketplace allowlists the target and the target is already
    // registered on the host, so an undeclared target would surface only as
    // an install error on every user's machine.
    pub(crate) fn validate_marketplace_dependencies(
        &self,
        name: &str,
        marketplace: &MarketplaceConfig,
    ) -> Result<(), ServicesValidationError> {
        let members = self.marketplace_plugin_configs(marketplace);
        for plugin in &members {
            for dependency in &plugin.dependencies {
                let Some(target) = dependency.marketplace.as_deref() else {
                    let carried = members.iter().any(|p| p.id.as_str() == dependency.name)
                        || marketplace
                            .external_plugins
                            .iter()
                            .any(|p| p.name == dependency.name);
                    if !carried {
                        return Err(ServicesValidationError::unknown_reference(
                            ValidationSubject::named("Marketplace", name),
                            format!(
                                "plugin '{}' depends on '{}', which this \
                             marketplace does not carry — name its marketplace, add it to \
                             plugins.include or list it under external_plugins",
                                plugin.id.as_str(),
                                dependency.name
                            ),
                        ));
                    }
                    continue;
                };
                if target == marketplace.id.as_str() {
                    continue;
                }
                if !marketplace
                    .allow_cross_marketplace_dependencies_on
                    .iter()
                    .any(|allowed| allowed == target)
                {
                    return Err(ServicesValidationError::business_rule(
                        ValidationSubject::named("Marketplace", name),
                        format!(
                            "plugin '{}' depends on '{}@{target}' but \
                         '{target}' is not in allow_cross_marketplace_dependencies_on",
                            plugin.id.as_str(),
                            dependency.name
                        ),
                    ));
                }
                let local = self.marketplaces.keys().any(|id| id.as_str() == target);
                if !local && marketplace.external_marketplace(target).is_none() {
                    return Err(ServicesValidationError::unknown_reference(
                        ValidationSubject::named("Marketplace", name),
                        format!(
                            "plugin '{}' depends on '{}@{target}' but \
                         '{target}' is neither a configured marketplace nor declared under \
                         external_marketplaces",
                            plugin.id.as_str(),
                            dependency.name
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_plugin_bindings(
        &self,
        plugin_name: &str,
        plugin: &PluginConfig,
    ) -> Result<(), ServicesValidationError> {
        if plugin.skills.source == ComponentSource::Explicit {
            for skill_ref in &plugin.skills.include {
                if !self.skills.skills.contains_key(skill_ref) {
                    return Err(ServicesValidationError::unknown_reference(
                        ValidationSubject::named("Plugin", plugin_name),
                        format!(
                            "skills.include references unknown skill \
                         '{skill_ref}'"
                        ),
                    ));
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
                    return Err(ServicesValidationError::unknown_reference(
                        ValidationSubject::named("Plugin", plugin_name),
                        format!(
                            "mcp_servers.include references unknown \
                         mcp_server '{mcp_ref}'"
                        ),
                    ));
                },
                Some(deployment) if plugin.enabled && !deployment.enabled => {
                    return Err(ServicesValidationError::business_rule(
                        ValidationSubject::clause("Plugin", plugin_name),
                        format!(
                            "is enabled but depends on disabled mcp_server \
                         '{mcp_ref}' — enable the server or disable the plugin"
                        ),
                    ));
                },
                Some(_) => {},
            }
        }

        for agent_ref in &plugin.agents.include {
            if !self.agents.contains_key(agent_ref) {
                return Err(ServicesValidationError::unknown_reference(
                    ValidationSubject::named("Plugin", plugin_name),
                    format!("agents.include references unknown agent '{agent_ref}'"),
                ));
            }
        }

        Ok(())
    }
}
