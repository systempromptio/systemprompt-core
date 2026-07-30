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

/// Whether the standalone `claude` CLI is installed on this machine.
///
/// `~/.claude` is created by the CLI's *first run*, not by its installation, so
/// testing for that directory alone reports "absent" on a freshly provisioned
/// machine where the tool is installed but has never been launched — which is
/// exactly the state a new user is in when the bridge first syncs. Probing the
/// binary on `PATH` is what distinguishes genuinely-absent from
/// installed-but-unused; the directory is still honoured because a user may
/// have the CLI on a path this process cannot see.
fn claude_cli_installed() -> bool {
    if paths::claude_cli_home().is_some_and(|h| h.exists()) {
        return true;
    }
    binary_on_path("claude")
}

fn binary_on_path(binary: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| {
        let candidate = dir.join(binary);
        candidate.is_file() || candidate.with_extension("exe").is_file()
    })
}

fn apply_install(ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
    let Some(plugins) = paths::claude_cli_plugins_dir() else {
        tracing::warn!(
            target: "bridge::claude-code-cli",
            "skipped: no home directory could be resolved, so ~/.claude/plugins has no location — \
             org plugins will NOT appear in `claude plugin list`"
        );
        return Ok(());
    };
    if !claude_cli_installed() {
        tracing::info!(
            target: "bridge::claude-code-cli",
            probed_path_for = "claude",
            "skipped: the standalone Claude Code CLI is not installed (no `claude` on PATH and no \
             ~/.claude); install it and re-run `sync` to receive org plugins"
        );
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
        tracing::warn!(
            target: "bridge::claude-code-cli",
            "clear skipped: no home directory could be resolved"
        );
        return Ok(());
    };
    // Clear is purely subtractive, so an absent ~/.claude genuinely means there
    // is nothing to undo. Without this, `set_enabled(&[])` below would create an
    // empty settings.json on a machine that has no Claude Code at all.
    if !paths::claude_cli_home().is_some_and(|h| h.exists()) {
        return Ok(());
    }
    remove_dir(&plugins.join("cache").join(MARKETPLACE))?;
    remove_dir(&marketplace_dir(&plugins))?;
    strip_installed_plugins(&plugins)?;
    strip_known_marketplace(&plugins)?;
    set_enabled(&[])?;
    Ok(())
}
