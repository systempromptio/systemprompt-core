//! Cowork desktop plugin integration.
//!
//! Cowork's filesystem plugin scanner discovers any plugin under
//! `%ProgramFiles%\Claude\org-plugins\<name>\` (and the equivalent per-OS path
//! resolved by [`crate::config::paths::org_plugins_effective`]) and attributes
//! it to the hard-coded `org-provisioned` marketplace. The bridge therefore
//! only owes Cowork one write per session: setting
//! `enabledPlugins["<plugin>@org-provisioned"] = true` in the per-session
//! `cowork_settings.json`. Everything else — copying the plugin tree, writing
//! `plugin.json` with `installationPreference: "auto_install"`, materialising
//! hooks — happens earlier in the `sync::apply::synthetic_plugin` flow that
//! populates the org-plugins root itself.
//!
//! Pure data manipulation lives in the `settings` submodule; IO (`emit`) and
//! the per-file upsert plumbing layer on top.

pub(crate) mod emit;
pub(crate) mod settings;
mod upsert;

pub use emit::{
    CoworkTarget, EmitReport, PERSONAL_SESSION_UUID, apply_enable, clear_all, pick_target,
    resolve_target,
};

pub use settings::{
    SettingsReport, disable_plugin, enable_plugin, enabled_plugins_key, parse_settings,
    render_settings,
};

// Lives outside `cowork_plugins/` (in `<session>/<org>/`) — Cowork resolves
// `enabledPlugins` from there, not from inside any registry tree.
pub(crate) const COWORK_SETTINGS_FILE: &str = "cowork_settings.json";

use thiserror::Error;

use async_trait::async_trait;

use crate::config::paths;
use crate::sync::ApplyError;
use crate::sync::host_sync::{HostSync, HostSyncCtx};

#[derive(Clone, Copy, Debug)]
pub struct CoworkSync;

#[async_trait]
impl HostSync for CoworkSync {
    fn host_id(&self) -> &'static str {
        "cowork"
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        let Some(target) = resolve_target() else {
            tracing::info!(
                target: "bridge::cowork",
                "no Cowork install detected; skipping enable"
            );
            return Ok(());
        };
        let plugin_name = paths::SYNTHETIC_PLUGIN_NAME;
        let report = apply_enable(&target, plugin_name).map_err(|e| ApplyError::Io {
            context: format!("cowork enable: {e}"),
            source: std::io::Error::other(e.to_string()),
        })?;
        tracing::info!(
            target: "bridge::cowork",
            session_org = ?report.target,
            enabled = report.enabled,
            "Cowork enable complete"
        );
        Ok(())
    }

    fn clear(&self) -> Result<(), ApplyError> {
        let Some(target) = resolve_target() else {
            return Ok(());
        };
        clear_all(&target, paths::SYNTHETIC_PLUGIN_NAME).map_err(|e| ApplyError::Io {
            context: format!("cowork clear: {e}"),
            source: std::io::Error::other(e.to_string()),
        })
    }
}

#[derive(Debug, Error)]
pub enum CoworkPluginsError {
    #[error("json parse failed: {0}")]
    JsonParse(#[from] serde_json::Error),
    #[error("cowork_settings.json root must be a JSON object")]
    RootShape,
    #[error("cowork_settings.json `{key}` must be a JSON object")]
    ItemsShape { key: &'static str },
}
