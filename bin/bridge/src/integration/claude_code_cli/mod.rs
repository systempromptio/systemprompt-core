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

pub const MARKETPLACE: &str = "org-provisioned";
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

pub(crate) fn marketplace_dir(plugins: &Path) -> PathBuf {
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

// Why: `~/.claude` is created by the CLI's first run, not its installation, so
// the PATH probe is what distinguishes genuinely-absent from
// installed-but-unused.
pub(crate) fn claude_cli_installed() -> bool {
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

    // Why: `managed-mcp.json` suppresses plugin-provided servers, so writing both
    // it and per-plugin `.mcp.json` files leaves the latter inert and misleading.
    let enforced = crate::install::managed_mcp::apply_policy(manifest.allow_claude_ai_connectors)
        == crate::install::managed_mcp::PolicyOutcome::Enforced;

    let mut ids = Vec::with_capacity(manifest.plugins.len());
    let mut entries = Vec::with_capacity(manifest.plugins.len());
    for plugin in &manifest.plugins {
        let id = plugin.id.as_str();
        let src = ctx.org_plugins_root.join(id);
        let mcp_servers = if enforced {
            &[][..]
        } else {
            ctx.plugin_mcp_servers
                .get(id)
                .map_or(&[][..], Vec::as_slice)
        };
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
        mcp_policy = if enforced { "managed" } else { "per-plugin" },
        "installed and enabled org plugins for the standalone Claude Code CLI"
    );
    Ok(())
}

pub(crate) fn clear_install() -> Result<(), ApplyError> {
    crate::install::managed_mcp::clear_policy();
    let Some(plugins) = paths::claude_cli_plugins_dir() else {
        tracing::warn!(
            target: "bridge::claude-code-cli",
            "clear skipped: no home directory could be resolved"
        );
        return Ok(());
    };
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
