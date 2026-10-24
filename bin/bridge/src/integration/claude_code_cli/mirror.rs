//! Mirrors one marketplace's plugins into `~/.claude` and removes a mirrored
//! marketplace again.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::bridge::host::HostKind;

use super::bundle::{mirror_plugin, remove_dir, remove_stale_children};
use super::foreign::{self, ForeignRefs};
use super::installed::{strip_installed_plugins, upsert_installed_plugins};
use super::layout::{cache_dir, cache_install_dir, marketplace_dir, source_plugin_dir};
use super::marketplace::{
    self, HostMarketplace, Mirrored, strip_known_marketplace, upsert_known_marketplace,
    write_marketplace_json,
};
use super::mcp;
use crate::host_sync::{ApplyError, HostSyncCtx, HostSyncReport};
use crate::ids::PluginId;

pub(super) fn mirror_marketplace(
    ctx: &HostSyncCtx<'_>,
    report: &mut HostSyncReport,
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
            report.warn(
                crate::host_sync::HostWarningKind::Manifest,
                HostKind::ClaudeCode,
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

    let dirs = marketplace.retained(&ids);
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
    upsert_installed_plugins(plugins, manifest, marketplace, &ids)?;
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

pub(super) fn purge_marketplace(
    plugins: &Path,
    marketplace: &MarketplaceId,
) -> Result<(), ApplyError> {
    remove_dir(&cache_dir(plugins, marketplace))?;
    remove_dir(&marketplace_dir(plugins, marketplace))?;
    strip_installed_plugins(plugins, marketplace)?;
    strip_known_marketplace(plugins, marketplace)?;
    Ok(())
}
