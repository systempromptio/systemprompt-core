//! Standalone Claude Code CLI sync emitter.
//!
//! The `claude` CLI does not read the Cowork org-plugins root, so this mirrors
//! every org plugin into `~/.claude` as a standard directory-source marketplace
//! (`marketplace.json` + one plugin dir per manifest plugin + cache bundles +
//! `known_marketplaces` + `installed_plugins`) and force-enables each plugin in
//! `settings.json`, preserving every foreign key. Result: each plugin appears
//! in `claude plugin list` and its skills load as `/<plugin-id>:<skill>`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bundle;
pub mod json_io;
pub mod marketplace;

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use bundle::{mirror_plugin, remove_dir, remove_stale_children};
use marketplace::{
    set_enabled, strip_installed_plugins, strip_known_marketplace, upsert_installed_plugins,
    upsert_known_marketplace, write_marketplace_json,
};

use crate::config::paths;
use crate::sync::ApplyError;
use crate::sync::host_sync::{HostSync, HostSyncCtx};

const MARKETPLACE: &str = "org-provisioned";
const VERSION_DIR: &str = "current";

pub(crate) struct ClaudeCodeCliSync;

#[async_trait]
impl HostSync for ClaudeCodeCliSync {
    fn host_id(&self) -> &'static str {
        "claude-code"
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        apply_install(ctx)
    }

    fn clear(&self) -> Result<(), ApplyError> {
        clear_install()
    }
}

fn plugin_key(plugin_id: &str) -> String {
    format!("{plugin_id}@{MARKETPLACE}")
}

fn marketplace_dir(plugins: &Path) -> PathBuf {
    plugins.join("marketplaces").join(MARKETPLACE)
}

fn source_plugin_dir(plugins: &Path, plugin_id: &str) -> PathBuf {
    marketplace_dir(plugins).join("plugins").join(plugin_id)
}

fn cache_install_dir(plugins: &Path, plugin_id: &str) -> PathBuf {
    plugins
        .join("cache")
        .join(MARKETPLACE)
        .join(plugin_id)
        .join(VERSION_DIR)
}

fn io_err(context: impl Into<String>, source: std::io::Error) -> ApplyError {
    ApplyError::Io {
        context: context.into(),
        source,
    }
}

fn apply_install(ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
    // `None` (no home) or no `~/.claude` means the standalone CLI isn't present;
    // treat as a no-op rather than materialising the tree for a missing tool.
    let Some(plugins) = paths::claude_cli_plugins_dir() else {
        return Ok(());
    };
    if !paths::claude_cli_home().is_some_and(|h| h.exists()) {
        return Ok(());
    }

    let manifest = ctx.manifest;
    if manifest.plugins.is_empty() {
        return clear_install();
    }

    let mut ids = Vec::with_capacity(manifest.plugins.len());
    let mut entries = Vec::with_capacity(manifest.plugins.len());
    for plugin in &manifest.plugins {
        let id = plugin.id.as_str();
        let src = ctx.org_plugins_root.join(id);
        let mcp_servers = ctx
            .plugin_mcp_servers
            .get(id)
            .map_or(&[][..], Vec::as_slice);
        mirror_plugin(&src, &source_plugin_dir(&plugins, id), mcp_servers)?;
        mirror_plugin(&src, &cache_install_dir(&plugins, id), mcp_servers)?;
        entries.push(marketplace::entry_for(&src, id, &plugin.version));
        ids.push(id);
    }

    remove_stale_children(&marketplace_dir(&plugins).join("plugins"), &ids)?;
    remove_stale_children(&plugins.join("cache").join(MARKETPLACE), &ids)?;

    write_marketplace_json(&plugins, manifest.manifest_version.as_str(), &entries)?;
    upsert_known_marketplace(&plugins, &manifest.issued_at)?;
    upsert_installed_plugins(&plugins, manifest, &ids)?;
    set_enabled(&ids)?;
    tracing::info!(
        target: "bridge::claude-code-cli",
        marketplace = MARKETPLACE,
        plugins = ids.len(),
        "installed and enabled org plugins for the standalone Claude Code CLI"
    );
    Ok(())
}

fn clear_install() -> Result<(), ApplyError> {
    let Some(plugins) = paths::claude_cli_plugins_dir() else {
        return Ok(());
    };
    remove_dir(&plugins.join("cache").join(MARKETPLACE))?;
    remove_dir(&marketplace_dir(&plugins))?;
    strip_installed_plugins(&plugins)?;
    strip_known_marketplace(&plugins)?;
    set_enabled(&[])?;
    Ok(())
}
