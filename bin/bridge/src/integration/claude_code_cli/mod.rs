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
mod mirror;
mod permissions;
pub mod sidecar;


use async_trait::async_trait;
use foreign::ForeignRefs;
use systemprompt_identifiers::MarketplaceId;

pub use bundle::filter_skills_for_host;
pub(crate) use layout::{feedback_skill_roots, marketplace_dir};
use marketplace::set_enabled;
pub use marketplace::{HostMarketplace, Mirrored, host_marketplaces};
use mirror::{mirror_marketplace, purge_marketplace};

use systemprompt_models::bridge::host::HostKind;

use crate::integration::claude_code_routing::Routing;

use crate::config::paths;
use crate::host_sync::{ApplyError, HostSync, HostSyncCtx, HostSyncReport};

pub(crate) struct ClaudeCodeCliSync;

#[async_trait]
impl HostSync for ClaudeCodeCliSync {
    fn host_id(&self) -> HostKind {
        HostKind::ClaudeCode
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<HostSyncReport, ApplyError> {
        apply_install(ctx)
    }

    fn clear(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        clear_install(ctx.elevation_gate)
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
    crate::sysproc::binary_on_path("claude").is_some()
}

fn apply_install(ctx: &HostSyncCtx<'_>) -> Result<HostSyncReport, ApplyError> {
    let mut report = HostSyncReport::ok();
    let Some(plugins) = paths::claude_cli_plugins_dir() else {
        tracing::warn!(
            target: "bridge::claude-code-cli",
            "skipped: no home directory could be resolved, so ~/.claude/plugins has no location — \
             org plugins will NOT appear in `claude plugin list`"
        );
        return Ok(report);
    };
    if !claude_cli_installed() {
        tracing::info!(
            target: "bridge::claude-code-cli",
            probed_path_for = "claude",
            "skipped: the standalone Claude Code CLI is not installed (no `claude` on PATH and no \
             ~/.claude); install it and re-run `sync` to receive org plugins"
        );
        return Ok(report);
    }

    ensure_routed(ctx)?;

    let manifest = ctx.manifest;
    let marketplaces = host_marketplaces(manifest);
    if marketplaces.is_empty() {
        if !manifest.plugins.is_empty() {
            report.warn(
                crate::host_sync::HostWarningKind::Manifest,
                HostKind::ClaudeCode,
                "the manifest carries plugins but names no marketplace; nothing was mirrored \
                 for the Claude Code CLI — upgrade the gateway",
            );
        }
        clear_install(ctx.elevation_gate)?;
        return Ok(report);
    }

    crate::install::managed_mcp::clear_policy(ctx.elevation_gate).map_err(|source| {
        ApplyError::Io {
            context: "remove managed MCP policy".to_owned(),
            source,
        }
    })?;

    let current: Vec<MarketplaceId> = marketplaces.iter().map(|m| m.id.clone()).collect();
    let mut mirrored = Vec::with_capacity(marketplaces.len());
    let mut foreign = ForeignRefs::default();
    for marketplace in &marketplaces {
        let (done, refs) = mirror_marketplace(ctx, &mut report, &plugins, marketplace, &current)?;
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
    report.merge(permissions::apply_client_settings(ctx)?);

    tracing::info!(
        target: "bridge::claude-code-cli",
        marketplaces = ?marketplaces.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        plugins = mirrored.iter().map(|m| m.plugin_ids.len()).sum::<usize>(),
        "installed and enabled org plugins for the standalone Claude Code CLI"
    );
    Ok(report)
}

// Why: Claude Code with no gateway keys in the settings it reads keeps the
// user's own Anthropic login, and a helper naming a bridge executable that a
// reinstall moved is set but dead. Only settings whose effective keys differ
// from what this bridge would write are rewritten.
fn ensure_routed(ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
    let origin = ctx.loopback.origin();
    match crate::integration::claude_code_routing::current_routing(Some(&origin)) {
        Routing::Routed => return Ok(()),
        Routing::Stale(key) => tracing::warn!(
            target: "bridge::claude-code-cli",
            stale = key.as_str(),
            "Claude Code settings name a stale gateway value; rewriting them"
        ),
        Routing::Unrouted => {},
    }
    crate::integration::enrol::claude_code::route(&origin).map_err(|source| ApplyError::Step {
        context: "route Claude Code through the gateway",
        source: Box::new(source),
    })
}

pub(crate) fn clear_install(gate: &crate::elevation_gate::ElevationGate) -> Result<(), ApplyError> {
    crate::install::managed_mcp::clear_policy(gate).map_err(|source| ApplyError::Io {
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
    permissions::clear_client_settings()?;
    sidecar::remove(&plugins)
}

crate::register_host_sync!(ClaudeCodeCliSync);
