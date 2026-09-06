//! Standalone Claude Code CLI sync emitter.
//!
//! The `claude` CLI does not read the Cowork org-plugins root, so this mirrors
//! every org plugin into `~/.claude` as standard directory-source marketplaces
//! — one per marketplace the gateway manifest lists, each holding
//! `marketplace.json` + one plugin dir per member plugin + cache bundles +
//! `known_marketplaces` + `installed_plugins` — and force-enables each plugin
//! in `settings.json` as `<plugin>@<marketplace-id>`, preserving every foreign
//! key. A plugin two marketplaces carry is mirrored under each. Result: each
//! plugin appears in `claude plugin list` and its skills load as
//! `/<plugin-id>:<skill>`.
//!
//! The marketplaces this emitter owns are recorded in a sidecar
//! ([`sidecar`]) so a later sync prunes only those and a marketplace the user
//! registered themselves is never touched. A manifest from a gateway older
//! than the `marketplaces` field lists none, and is mirrored as the single
//! [`LEGACY_MARKETPLACE`] holding every plugin — the layout every bridge wrote
//! before — so behaviour on such a gateway is unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bundle;
pub mod json_io;
pub mod marketplace;
pub mod sidecar;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use systemprompt_identifiers::MarketplaceId;

use bundle::{mirror_plugin, remove_dir, remove_stale_children};
use marketplace::{
    set_enabled, strip_installed_plugins, strip_known_marketplace, upsert_installed_plugins,
    upsert_known_marketplace, write_marketplace_json,
};

use crate::config::paths;
use crate::gateway::manifest::SignedManifest;
use crate::host_sync::{ApplyError, HostSync, HostSyncCtx};
use crate::ids::PluginId;

// Why: the one marketplace every bridge wrote before the manifest named its
// marketplaces. It is still the shape an older gateway is mirrored as, and the
// key a first sync against a newer gateway purges.
pub const LEGACY_MARKETPLACE: &str = "org-provisioned";
const LEGACY_DESCRIPTION: &str =
    "Skills, agents, and MCP servers provisioned by your organization.";
const VERSION_DIR: &str = "current";

#[must_use]
pub fn legacy_marketplace_id() -> MarketplaceId {
    MarketplaceId::new(LEGACY_MARKETPLACE)
}

/// A Claude Code marketplace this emitter mirrors: the gateway marketplace's
/// id and name, and the manifest plugins it carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostMarketplace {
    pub id: MarketplaceId,
    pub name: String,
    pub plugin_ids: Vec<PluginId>,
}

/// The plugins mirrored under one marketplace, as written to `settings.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mirrored {
    pub id: MarketplaceId,
    pub plugin_ids: Vec<PluginId>,
}

pub(crate) struct ClaudeCodeCliSync;

#[async_trait]
impl HostSync for ClaudeCodeCliSync {
    fn host_id(&self) -> &'static str {
        "claude-code"
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        apply_install(ctx)
    }

    fn clear(&self, _ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        clear_install()
    }
}

#[must_use]
pub fn host_marketplaces(manifest: &SignedManifest) -> Vec<HostMarketplace> {
    if manifest.plugins.is_empty() {
        return Vec::new();
    }
    if manifest.marketplaces.is_empty() {
        return vec![HostMarketplace {
            id: legacy_marketplace_id(),
            name: LEGACY_DESCRIPTION.to_owned(),
            plugin_ids: manifest.plugins.iter().map(|p| p.id.clone()).collect(),
        }];
    }
    manifest
        .marketplaces
        .iter()
        .map(|m| HostMarketplace {
            id: m.id.clone(),
            name: m.name.clone(),
            plugin_ids: m.plugin_ids.clone(),
        })
        .collect()
}

pub(crate) fn plugin_key(plugin_id: &PluginId, marketplace: &MarketplaceId) -> String {
    format!("{}@{}", plugin_id.as_str(), marketplace.as_str())
}

pub(crate) fn marketplace_dir(plugins: &Path, marketplace: &MarketplaceId) -> PathBuf {
    plugins.join("marketplaces").join(marketplace.as_str())
}

fn source_plugin_dir(plugins: &Path, marketplace: &MarketplaceId, plugin_id: &PluginId) -> PathBuf {
    marketplace_dir(plugins, marketplace)
        .join("plugins")
        .join(plugin_id.as_str())
}

fn cache_dir(plugins: &Path, marketplace: &MarketplaceId) -> PathBuf {
    plugins.join("cache").join(marketplace.as_str())
}

fn cache_install_dir(plugins: &Path, marketplace: &MarketplaceId, plugin_id: &PluginId) -> PathBuf {
    cache_dir(plugins, marketplace)
        .join(plugin_id.as_str())
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
    let marketplaces = host_marketplaces(manifest);
    if marketplaces.is_empty() {
        return clear_install();
    }

    // Why: an enterprise MCP policy left by an older bridge shadows every
    // plugin-provided server and Cowork's own tools; it goes before the
    // per-plugin `.mcp.json` files are written so those are the servers the
    // CLI actually loads.
    crate::install::managed_mcp::clear_policy();

    let mut mirrored = Vec::with_capacity(marketplaces.len());
    for marketplace in &marketplaces {
        mirrored.push(mirror_marketplace(ctx, &plugins, marketplace)?);
    }

    let current: Vec<MarketplaceId> = mirrored.iter().map(|m| m.id.clone()).collect();
    let stale: Vec<MarketplaceId> = sidecar::owned_marketplaces(&plugins, sidecar::Legacy::Always)?
        .into_iter()
        .filter(|id| !current.contains(id))
        .collect();
    for id in &stale {
        purge_marketplace(&plugins, id)?;
    }
    set_enabled(&mirrored, &stale)?;
    sidecar::write(&plugins, &current)?;

    tracing::info!(
        target: "bridge::claude-code-cli",
        marketplaces = ?current.iter().map(MarketplaceId::as_str).collect::<Vec<_>>(),
        plugins = mirrored.iter().map(|m| m.plugin_ids.len()).sum::<usize>(),
        "installed and enabled org plugins for the standalone Claude Code CLI"
    );
    Ok(())
}

fn mirror_marketplace(
    ctx: &HostSyncCtx<'_>,
    plugins: &Path,
    marketplace: &HostMarketplace,
) -> Result<Mirrored, ApplyError> {
    let manifest = ctx.manifest;
    let versions: BTreeMap<&str, &str> = manifest
        .plugins
        .iter()
        .map(|p| (p.id.as_str(), p.version.as_str()))
        .collect();

    let mut ids: Vec<&PluginId> = Vec::with_capacity(marketplace.plugin_ids.len());
    let mut entries = Vec::with_capacity(marketplace.plugin_ids.len());
    for id in &marketplace.plugin_ids {
        // Why: the gateway never lists a plugin the manifest lacks; if one ever
        // arrives there is nothing on disk to mirror, so it is skipped rather
        // than failing the whole host.
        let Some(version) = versions.get(id.as_str()) else {
            continue;
        };
        let src = ctx.org_plugins_root.join(id.as_str());
        let mcp_servers = ctx
            .plugin_mcp_servers
            .get(id.as_str())
            .map_or(&[][..], Vec::as_slice);
        mirror_plugin(
            ctx.loopback,
            &src,
            &source_plugin_dir(plugins, &marketplace.id, id),
            mcp_servers,
        )?;
        mirror_plugin(
            ctx.loopback,
            &src,
            &cache_install_dir(plugins, &marketplace.id, id),
            mcp_servers,
        )?;
        entries.push(marketplace::entry_for(&src, id, version));
        ids.push(id);
    }

    let dirs: Vec<&str> = ids.iter().map(|id| id.as_str()).collect();
    remove_stale_children(
        &marketplace_dir(plugins, &marketplace.id).join("plugins"),
        &dirs,
    )?;
    remove_stale_children(&cache_dir(plugins, &marketplace.id), &dirs)?;

    write_marketplace_json(
        plugins,
        marketplace,
        manifest.manifest_version.as_str(),
        &entries,
    )?;
    upsert_known_marketplace(plugins, &marketplace.id, &manifest.issued_at)?;
    upsert_installed_plugins(plugins, manifest, &marketplace.id, &ids)?;
    Ok(Mirrored {
        id: marketplace.id.clone(),
        plugin_ids: ids.into_iter().cloned().collect(),
    })
}

fn purge_marketplace(plugins: &Path, marketplace: &MarketplaceId) -> Result<(), ApplyError> {
    remove_dir(&cache_dir(plugins, marketplace))?;
    remove_dir(&marketplace_dir(plugins, marketplace))?;
    strip_installed_plugins(plugins, marketplace)?;
    strip_known_marketplace(plugins, marketplace)?;
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
    let owned = sidecar::owned_marketplaces(&plugins, sidecar::Legacy::Always)?;
    for id in &owned {
        purge_marketplace(&plugins, id)?;
    }
    set_enabled(&[], &owned)?;
    sidecar::remove(&plugins)
}

crate::register_host_sync!(ClaudeCodeCliSync);
