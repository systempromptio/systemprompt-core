//! IO layer for the Cowork desktop integration: writes the org-provisioned
//! enable key in `cowork_settings.json` and purges legacy session-marketplace
//! state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::{fs, io};


pub(super) const ORG_PROVISIONED_MARKETPLACE: &str = "org-provisioned";

pub(super) const INSTALLED_PLUGINS_FILE: &str = "installed_plugins.json";

use super::CoworkPluginsError;
use super::upsert::reconcile_enabled;
use crate::fsutil;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum EmitError {
    #[error("io error in {context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
    #[error("data error: {0}")]
    Data(#[from] CoworkPluginsError),
}

#[derive(Debug, Clone)]
pub struct CoworkTarget {
    pub session_org_dir: PathBuf,
    pub cowork_plugins_dir: PathBuf,
}

#[derive(Debug, Default, Clone)]
pub struct EmitReport {
    pub target: Option<PathBuf>,
    pub enabled: bool,
}

pub fn apply_enable(target: &CoworkTarget, plugin_ids: &[&str]) -> Result<EmitReport, EmitError> {
    super::prune::prune_orphans(target, plugin_ids, ORG_PROVISIONED_MARKETPLACE)?;
    reconcile_enabled(target, plugin_ids, ORG_PROVISIONED_MARKETPLACE)?;
    tracing::info!(
        target: "bridge::cowork",
        session_org = %target.session_org_dir.display(),
        plugins = plugin_ids.len(),
        marketplace = ORG_PROVISIONED_MARKETPLACE,
        "reconciled bridge plugin enables in Cowork settings"
    );
    Ok(EmitReport {
        target: Some(target.session_org_dir.clone()),
        enabled: !plugin_ids.is_empty(),
    })
}

pub fn clear_all(target: &CoworkTarget) -> Result<(), EmitError> {
    reconcile_enabled(target, &[], ORG_PROVISIONED_MARKETPLACE)?;
    Ok(())
}

pub(super) fn remove_tree(path: &std::path::Path) -> Result<bool, EmitError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(EmitError::Io {
            context: format!("remove_dir_all {}", path.display()),
            source: e,
        }),
    }
}

pub(super) fn strip_nested_object_key(
    path: &std::path::Path,
    parent: &str,
    key: &str,
) -> Result<bool, EmitError> {
    let Some(text) = fsutil::read_optional(path).map_err(|e| EmitError::Io {
        context: format!("read {}", path.display()),
        source: e,
    })?
    else {
        return Ok(false);
    };
    let mut root: Value = serde_json::from_str(&text).map_err(CoworkPluginsError::from)?;
    let Some(inner) = root
        .as_object_mut()
        .and_then(|m| m.get_mut(parent))
        .and_then(Value::as_object_mut)
    else {
        return Ok(false);
    };
    if inner.remove(key).is_none() {
        return Ok(false);
    }
    let bytes = serde_json::to_vec_pretty(&root).map_err(CoworkPluginsError::from)?;
    fsutil::atomic_write_0600(path, &bytes).map_err(|e| EmitError::Io {
        context: format!("atomic_write {}", path.display()),
        source: e,
    })?;
    Ok(true)
}
