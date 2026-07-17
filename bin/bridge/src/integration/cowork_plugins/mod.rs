//! Cowork desktop plugin integration: one write per session enabling the
//! synthetic plugin in `cowork_settings.json`. Pure data in `settings`; IO in
//! `emit`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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

    async fn apply(&self, _ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
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
