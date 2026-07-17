//! Standalone Claude Code CLI sync emitter.
//!
//! The `claude` CLI does not read the Cowork org-plugins root, so this installs
//! the org plugin into `~/.claude` as a standard directory-source marketplace
//! plugin (`marketplace.json` + cache bundle + `known_marketplaces` +
//! `installed_plugins`) and force-enables it in `settings.json`, preserving
//! every foreign key. Result: it appears in `claude plugin list` and its skills
//! load
//! as `/systemprompt-managed:<skill>`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bundle;
pub mod json_io;
pub mod marketplace;

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use bundle::{remove_dir, write_bundle};
use marketplace::{
    set_enabled, strip_installed_plugin, strip_known_marketplace, upsert_installed_plugin,
    upsert_known_marketplace, write_marketplace_json,
};

use crate::config::paths;
use crate::gateway::manifest::SignedManifest;
use crate::sync::ApplyError;
use crate::sync::host_sync::{HostSync, HostSyncCtx};

const MARKETPLACE: &str = "org-provisioned";
const PLUGIN_NAME: &str = "systemprompt-managed";
const VERSION_DIR: &str = "current";

pub(crate) struct ClaudeCodeCliSync;

#[async_trait]
impl HostSync for ClaudeCodeCliSync {
    fn host_id(&self) -> &'static str {
        "claude-code"
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        apply_install(ctx.manifest)
    }

    fn clear(&self) -> Result<(), ApplyError> {
        clear_install()
    }
}

fn plugin_id() -> String {
    format!("{PLUGIN_NAME}@{MARKETPLACE}")
}

fn marketplace_dir(plugins: &Path) -> PathBuf {
    plugins.join("marketplaces").join(MARKETPLACE)
}

fn source_plugin_dir(plugins: &Path) -> PathBuf {
    marketplace_dir(plugins).join("plugins").join(PLUGIN_NAME)
}

fn cache_install_dir(plugins: &Path) -> PathBuf {
    plugins
        .join("cache")
        .join(MARKETPLACE)
        .join(PLUGIN_NAME)
        .join(VERSION_DIR)
}

fn io_err(context: impl Into<String>, source: std::io::Error) -> ApplyError {
    ApplyError::Io {
        context: context.into(),
        source,
    }
}

fn apply_install(manifest: &SignedManifest) -> Result<(), ApplyError> {
    // `None` (no home) or no `~/.claude` means the standalone CLI isn't present;
    // treat as a no-op rather than materialising the tree for a missing tool.
    let Some(plugins) = paths::claude_cli_plugins_dir() else {
        return Ok(());
    };
    if !paths::claude_cli_home().is_some_and(|h| h.exists()) {
        return Ok(());
    }

    let has_content = !manifest.skills.is_empty()
        || !manifest.agents.is_empty()
        || !manifest.managed_mcp_servers.is_empty();
    if !has_content {
        return clear_install();
    }

    let cache = cache_install_dir(&plugins);
    write_bundle(&cache, manifest)?;
    write_bundle(&source_plugin_dir(&plugins), manifest)?;
    write_marketplace_json(&plugins, manifest)?;
    upsert_known_marketplace(&plugins, &manifest.issued_at)?;
    upsert_installed_plugin(&plugins, &cache, manifest)?;
    set_enabled(true)?;
    tracing::info!(
        target: "bridge::claude-code-cli",
        plugin = %plugin_id(),
        "installed and enabled org plugin for the standalone Claude Code CLI"
    );
    Ok(())
}

fn clear_install() -> Result<(), ApplyError> {
    let Some(plugins) = paths::claude_cli_plugins_dir() else {
        return Ok(());
    };
    remove_dir(&cache_install_dir(&plugins))?;
    remove_dir(&marketplace_dir(&plugins))?;
    strip_installed_plugin(&plugins)?;
    strip_known_marketplace(&plugins)?;
    set_enabled(false)?;
    Ok(())
}
