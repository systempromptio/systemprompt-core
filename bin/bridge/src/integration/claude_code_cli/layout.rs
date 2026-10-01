//! Where the standalone Claude Code CLI keeps the marketplaces this emitter
//! mirrors: the directory-source marketplace, its plugin dirs and the cache
//! bundles `claude` installs from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use systemprompt_identifiers::MarketplaceId;

use crate::ids::PluginId;

const VERSION_DIR: &str = "current";

pub(crate) fn plugin_key(plugin_id: &PluginId, marketplace: &MarketplaceId) -> String {
    format!("{}@{}", plugin_id.as_str(), marketplace.as_str())
}

pub(crate) fn marketplace_dir(plugins: &Path, marketplace: &MarketplaceId) -> PathBuf {
    plugins.join("marketplaces").join(marketplace.as_str())
}

pub(super) fn source_plugin_dir(
    plugins: &Path,
    marketplace: &MarketplaceId,
    plugin_id: &PluginId,
) -> PathBuf {
    marketplace_dir(plugins, marketplace)
        .join("plugins")
        .join(plugin_id.as_str())
}

pub(super) fn cache_dir(plugins: &Path, marketplace: &MarketplaceId) -> PathBuf {
    plugins.join("cache").join(marketplace.as_str())
}

pub(super) fn cache_install_dir(
    plugins: &Path,
    marketplace: &MarketplaceId,
    plugin_id: &PluginId,
) -> PathBuf {
    cache_dir(plugins, marketplace)
        .join(plugin_id.as_str())
        .join(VERSION_DIR)
}

pub(crate) fn feedback_skill_roots(
    manifest: &crate::gateway::manifest::SignedManifest,
    skill: &crate::gateway::manifest::SkillEntry,
) -> Vec<PathBuf> {
    if !super::claude_cli_installed() {
        return Vec::new();
    }
    let Some(plugins) = crate::config::paths::claude_cli_plugins_dir() else {
        return Vec::new();
    };
    super::marketplace::host_marketplaces(manifest)
        .iter()
        .flat_map(|marketplace| {
            marketplace
                .plugin_ids
                .iter()
                .filter(|id| skill.plugins.contains(id))
                .map(|plugin| {
                    cache_install_dir(&plugins, &marketplace.id, plugin)
                        .join("skills")
                        .join(skill.id.as_str().replace('_', "-"))
                })
        })
        .collect()
}
