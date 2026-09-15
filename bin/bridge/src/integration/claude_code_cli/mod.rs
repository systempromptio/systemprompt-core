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
//! registered themselves is never touched. A manifest that carries plugins
//! but names no marketplace mirrors nothing and reports a host warning.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bundle;
pub mod foreign;
pub mod installed;
pub mod layout;
pub mod marketplace;
mod mcp;
mod permissions;
pub mod sidecar;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use foreign::ForeignRefs;
use systemprompt_identifiers::MarketplaceId;

pub use bundle::filter_skills_for_host;
use bundle::{mirror_plugin, remove_dir, remove_stale_children};
use installed::{strip_installed_plugins, upsert_installed_plugins};
pub(crate) use layout::marketplace_dir;
use layout::{cache_dir, cache_install_dir, source_plugin_dir};
pub use marketplace::{HostMarketplace, Mirrored, host_marketplaces};
use marketplace::{
    set_enabled, strip_known_marketplace, upsert_known_marketplace, write_marketplace_json,
};

use crate::config::paths;
use crate::gateway::manifest::SignedManifest;
use crate::host_sync::{ApplyError, HostSync, HostSyncCtx};
use crate::ids::PluginId;

pub const HOST_ID: &str = "claude-code";

pub(crate) struct ClaudeCodeCliSync;

#[async_trait]
impl HostSync for ClaudeCodeCliSync {
    fn host_id(&self) -> &'static str {
        HOST_ID
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        apply_install(ctx)
    }

    fn clear(&self, _ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        clear_install()
    }
}

fn io_err(context: impl Into<String>, source: std::io::Error) -> ApplyError {
    ApplyError::Io {
        context: context.into(),
        source,
    }
}

// Why: Claude Code creates ~/.claude on first run, not during installation.
pub(crate) fn claude_cli_installed() -> bool {
    if paths::claude_cli_home().is_some_and(|h| h.exists()) {
        return true;
    }
    crate::sync::apply::node_deps::binary_on_path("claude").is_some()
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
        if !manifest.plugins.is_empty() {
            ctx.warnings.push(
                HOST_ID,
                "the manifest carries plugins but names no marketplace; nothing was mirrored \
                 for the Claude Code CLI — upgrade the gateway",
            );
        }
        return clear_install();
    }

    crate::install::managed_mcp::clear_policy().map_err(|source| ApplyError::Io {
        context: "remove managed MCP policy".to_owned(),
        source,
    })?;

    let current: Vec<MarketplaceId> = marketplaces.iter().map(|m| m.id.clone()).collect();
    let mut mirrored = Vec::with_capacity(marketplaces.len());
    let mut foreign = ForeignRefs::default();
    for marketplace in &marketplaces {
        let (done, refs) = mirror_marketplace(ctx, &plugins, marketplace, &current)?;
        mirrored.push(done);
        foreign.extend(refs);
    }

    let previous = sidecar::read(&plugins)?;
    let stale: Vec<MarketplaceId> = previous
        .marketplaces
        .iter()
        .filter(|id| !current.contains(id))
        .cloned()
        .collect();
    for id in &stale {
        purge_marketplace(&plugins, id)?;
    }
    set_enabled(&mirrored, &stale, &previous, &foreign)?;
    sidecar::write(
        &plugins,
        &sidecar::Owned {
            marketplaces: current,
            dependency_keys: foreign.dependency_keys.iter().cloned().collect(),
            external_marketplaces: foreign.external_names(),
        },
    )?;
    permissions::apply_tool_permissions(ctx)?;

    tracing::info!(
        target: "bridge::claude-code-cli",
        marketplaces = ?marketplaces.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        plugins = mirrored.iter().map(|m| m.plugin_ids.len()).sum::<usize>(),
        "installed and enabled org plugins for the standalone Claude Code CLI"
    );
    Ok(())
}

fn mirror_marketplace(
    ctx: &HostSyncCtx<'_>,
    plugins: &Path,
    marketplace: &HostMarketplace,
    all_mirrored: &[MarketplaceId],
) -> Result<(Mirrored, ForeignRefs), ApplyError> {
    let manifest = ctx.manifest;
    let versions: BTreeMap<&str, &str> = manifest
        .plugins
        .iter()
        .map(|p| (p.id.as_str(), p.version.as_str()))
        .collect();

    let mut ids: Vec<&PluginId> = Vec::with_capacity(marketplace.plugin_ids.len());
    let mut entries = Vec::with_capacity(marketplace.plugin_ids.len());
    for id in &marketplace.plugin_ids {
        let Some(version) = versions.get(id.as_str()) else {
            ctx.warnings.push(
                HOST_ID,
                format!(
                    "marketplace {} lists plugin {} which the manifest does not carry; skipped",
                    marketplace.id.as_str(),
                    id.as_str()
                ),
            );
            continue;
        };
        let src = ctx.org_plugins_root.join(id.as_str());
        let mcp_servers = mcp::servers_for_plugin(ctx, id);
        mirror_plugin(
            ctx.loopback,
            &src,
            &source_plugin_dir(plugins, &marketplace.id, id),
            &mcp_servers,
            &ctx.manifest.skills,
        )?;
        mirror_plugin(
            ctx.loopback,
            &src,
            &cache_install_dir(plugins, &marketplace.id, id),
            &mcp_servers,
            &ctx.manifest.skills,
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
    upsert_known_marketplace(plugins, &marketplace.id, &manifest.issued_at.to_rfc3339())?;
    upsert_installed_plugins(plugins, manifest, &marketplace.id, &ids)?;
    let sources: Vec<PathBuf> = ids
        .iter()
        .map(|id| ctx.org_plugins_root.join(id.as_str()))
        .collect();
    let source_refs: Vec<&Path> = sources.iter().map(PathBuf::as_path).collect();
    let foreign = foreign::collect(marketplace, &source_refs, all_mirrored);
    Ok((
        Mirrored {
            id: marketplace.id.clone(),
            plugin_ids: ids.into_iter().cloned().collect(),
        },
        foreign,
    ))
}

fn purge_marketplace(plugins: &Path, marketplace: &MarketplaceId) -> Result<(), ApplyError> {
    remove_dir(&cache_dir(plugins, marketplace))?;
    remove_dir(&marketplace_dir(plugins, marketplace))?;
    strip_installed_plugins(plugins, marketplace)?;
    strip_known_marketplace(plugins, marketplace)?;
    Ok(())
}

pub(crate) fn clear_install() -> Result<(), ApplyError> {
    crate::install::managed_mcp::clear_policy().map_err(|source| ApplyError::Io {
        context: "remove managed MCP policy".to_owned(),
        source,
    })?;
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
    let owned = sidecar::read(&plugins)?;
    for id in &owned.marketplaces {
        purge_marketplace(&plugins, id)?;
    }
    set_enabled(&[], &owned.marketplaces, &owned, &ForeignRefs::default())?;
    permissions::clear_tool_permissions()?;
    sidecar::remove(&plugins)
}

crate::register_host_sync!(ClaudeCodeCliSync);

pub(crate) fn feedback_skill_roots(
    manifest: &SignedManifest,
    skill: &crate::gateway::manifest::SkillEntry,
) -> Vec<PathBuf> {
    if !claude_cli_installed() {
        return Vec::new();
    }
    let Some(plugins) = paths::claude_cli_plugins_dir() else {
        return Vec::new();
    };
    host_marketplaces(manifest)
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
