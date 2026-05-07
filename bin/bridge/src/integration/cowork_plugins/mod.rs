//! Cowork desktop marketplace + plugin store integration.
//!
//! Cowork loads plugins via a per-session marketplace registry under
//! `<session>/<org>/cowork_plugins/`. The bridge registers a single
//! `systemprompt-bridge-managed` marketplace there with `source: "local"`,
//! pointing at on-disk plugin content the bridge controls. This is the proper
//! marketplace mechanism (survives restart) — not a manifest patch.
//!
//! Pure data manipulation lives in the `registry`, `settings` and
//! `marketplace` submodules; IO (`emit`) and per-file upsert plumbing layer
//! on top.

pub(crate) mod emit;
pub(crate) mod marketplace;
pub(crate) mod registry;
pub(crate) mod settings;
mod upsert;

pub use emit::{publish, resolve_target, unpublish};

pub use marketplace::{
    MarketplaceFile, MarketplaceOwner, MarketplacePluginEntry, render_marketplace,
};
pub use registry::{
    InstalledPluginEntry, KnownMarketplaceEntry, LocalSource, MergeReport, parse_root,
    upsert_installed_plugin, upsert_known_marketplace,
};
pub use settings::{
    SettingsReport, disable_plugin, enable_plugin, enabled_plugins_key, parse_settings,
    render_settings,
};

// Lives outside `cowork_plugins/` (in `<session>/<org>/`) — Cowork resolves
// `enabledPlugins` from there, not from inside the registry tree.
pub(crate) const COWORK_SETTINGS_FILE: &str = "cowork_settings.json";

use thiserror::Error;

use crate::config::paths;
use crate::sync::ApplyError;
use crate::sync::host_sync::{HostSync, HostSyncCtx};

pub struct CoworkSync;

impl HostSync for CoworkSync {
    fn host_id(&self) -> &'static str {
        "cowork"
    }

    fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        let Some(target) = resolve_target() else {
            tracing::info!(
                target: "bridge::cowork",
                "no Cowork install detected; skipping marketplace emit"
            );
            return Ok(());
        };
        let plugin_name = paths::SYNTHETIC_PLUGIN_NAME;
        let version = ctx.manifest.manifest_version.as_str();
        let report = publish(
            &target,
            ctx.org_plugins_root,
            plugin_name,
            version,
            Some("Skills, agents, and MCP servers managed by your organization."),
        )
        .map_err(|e| ApplyError::Io {
            context: format!("cowork publish: {e}"),
            source: std::io::Error::other(e.to_string()),
        })?;
        tracing::info!(
            target: "bridge::cowork",
            session_org = ?report.target,
            copied = report.plugin_copied,
            registered_marketplace = report.marketplace_registered,
            registered_plugin = report.plugin_installed_registered,
            enabled = report.enabled,
            "Cowork marketplace emit complete"
        );
        Ok(())
    }

    fn clear(&self) -> Result<(), ApplyError> {
        let Some(target) = resolve_target() else {
            return Ok(());
        };
        unpublish(&target, paths::SYNTHETIC_PLUGIN_NAME).map_err(|e| ApplyError::Io {
            context: format!("cowork unpublish: {e}"),
            source: std::io::Error::other(e.to_string()),
        })
    }
}

pub const KNOWN_MARKETPLACES_FILE: &str = "known_marketplaces.json";

pub(crate) const INSTALLED_PLUGINS_FILE: &str = "installed_plugins.json";

#[derive(Debug, Error)]
pub enum CoworkPluginsError {
    #[error("json parse failed: {0}")]
    JsonParse(#[from] serde_json::Error),
    #[error("registry root must be a JSON object")]
    RootShape,
    #[error("registry items at key `{key}` must be a JSON array")]
    ItemsShape { key: &'static str },
}
