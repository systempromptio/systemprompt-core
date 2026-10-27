//! Marketplace membership selection over a resolved [`ServicesConfig`].
//!
//! Resolves which enabled plugins a marketplace carries and which skills those
//! plugins (and the agents they select) contribute.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use super::{ExternalAgentConfig, MarketplaceConfig, PluginConfig, ServicesConfig};
use systemprompt_models::bridge::host::{HostKind, InstanceHosts};
use systemprompt_models::plugin::ComponentSource;

impl ServicesConfig {
    fn external_agent(&self, host: HostKind) -> Option<&ExternalAgentConfig> {
        self.external_agents
            .iter()
            .find(|(id, _)| id.as_str().replace('_', "-") == host.as_str())
            .map(|(_, agent)| agent)
    }

    /// The hosts this instance serves. A host with no `external_agents` entry
    /// is served.
    #[must_use]
    pub fn instance_enabled_hosts(&self) -> Vec<HostKind> {
        HostKind::ALL
            .into_iter()
            .filter(|host| self.external_agent(*host).is_none_or(|agent| agent.enabled))
            .collect()
    }

    #[must_use]
    pub fn instance_hosts(&self) -> InstanceHosts {
        let enabled = self.instance_enabled_hosts();
        let opt_in = enabled
            .iter()
            .copied()
            .filter(|host| {
                self.external_agent(*host)
                    .is_some_and(|agent| !agent.default_enabled)
            })
            .collect();
        InstanceHosts { enabled, opt_in }
    }

    #[must_use]
    pub fn enabled_marketplaces(&self) -> Vec<&MarketplaceConfig> {
        let mut out: Vec<&MarketplaceConfig> =
            self.marketplaces.values().filter(|m| m.enabled).collect();
        out.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        out
    }

    #[must_use]
    pub fn marketplace_plugin_configs(
        &self,
        marketplace: &MarketplaceConfig,
    ) -> Vec<&PluginConfig> {
        let mut out: Vec<&PluginConfig> = self
            .plugins
            .values()
            .filter(|p| p.enabled)
            .filter(|p| {
                marketplace.plugins.include.is_empty()
                    || marketplace
                        .plugins
                        .include
                        .iter()
                        .any(|inc| inc == p.id.as_str())
            })
            .collect();
        out.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        out
    }

    #[must_use]
    pub fn plugin_selected_skill_ids(&self, plugin: &PluginConfig) -> BTreeSet<String> {
        let mut ids: BTreeSet<String> = match plugin.skills.source {
            ComponentSource::Explicit => plugin.skills.include.iter().cloned().collect(),
            ComponentSource::Instance => self
                .skills
                .skills
                .keys()
                .filter(|k| !plugin.skills.exclude.iter().any(|ex| ex == *k))
                .cloned()
                .collect(),
        };

        let selected_agent = |name: &str| match plugin.agents.source {
            ComponentSource::Explicit => plugin.agents.include.iter().any(|inc| inc == name),
            ComponentSource::Instance => !plugin.agents.exclude.iter().any(|ex| ex == name),
        };
        for (name, agent) in &self.agents {
            if selected_agent(name) {
                ids.extend(agent.metadata.skills.include.iter().cloned());
            }
        }

        ids
    }

    #[must_use]
    pub fn marketplace_skill_members(&self, marketplace: &MarketplaceConfig) -> BTreeSet<String> {
        self.marketplace_plugin_configs(marketplace)
            .into_iter()
            .flat_map(|plugin| self.plugin_selected_skill_ids(plugin))
            .collect()
    }
}
