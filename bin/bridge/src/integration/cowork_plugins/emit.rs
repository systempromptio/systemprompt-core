//! IO layer for the Cowork desktop integration: writes the org-provisioned
//! enable key in `cowork_settings.json` and purges legacy session-marketplace
//! state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::time::SystemTime;
use std::{fs, io};

use crate::config::paths;

// Cowork's fixed sentinel for the personal org-session dir; if it ever changes,
// `pick_target` falls back to mtime.
pub const PERSONAL_SESSION_UUID: &str = "00000000-0000-4000-8000-000000000001";

pub(super) const ORG_PROVISIONED_MARKETPLACE: &str = "org-provisioned";

// Legacy session-marketplace entries shadow the org-provisioned filesystem
// scan, so every `apply_enable` purges them.
const LEGACY_MARKETPLACE_TO_PURGE: &str = "systemprompt-bridge-managed";

// Pre-per-plugin bridges enabled one aggregate plugin under this name; its
// enable key and on-disk state are purged on every apply.
const LEGACY_SYNTHETIC_PLUGIN: &str = "systemprompt-managed";

pub(super) const INSTALLED_PLUGINS_FILE: &str = "installed_plugins.json";
const KNOWN_MARKETPLACES_FILE: &str = "known_marketplaces.json";

use super::CoworkPluginsError;
use super::upsert::{clear_enabled, reconcile_enabled};
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

/// `None` means no Cowork install detected; callers treat it as a no-op, not an
/// error.
#[must_use]
pub fn resolve_target() -> Option<CoworkTarget> {
    if let Some(configured) = configured_session_org_dir() {
        return Some(configured);
    }

    let sessions_root = paths::cowork3p_sessions_root()?;
    if !sessions_root.is_dir() {
        return None;
    }
    let mut candidates: Vec<(SystemTime, PathBuf)> = Vec::new();
    for session in fs::read_dir(&sessions_root).ok()?.flatten() {
        if !session.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let Ok(orgs) = fs::read_dir(session.path()) else {
            continue;
        };
        for org in orgs.flatten() {
            if !org.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let path = org.path();
            let mtime = fs::metadata(&path)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            candidates.push((mtime, path));
        }
    }

    let session_org_dir = pick_target(&candidates)?;
    let cowork_plugins_dir = session_org_dir.join(paths::COWORK_PLUGINS_SUBDIR);
    Some(CoworkTarget {
        session_org_dir,
        cowork_plugins_dir,
    })
}

fn configured_session_org_dir() -> Option<CoworkTarget> {
    let raw = crate::config::Config::load()
        .cowork
        .and_then(|c| c.session_org_dir)?;
    let path = PathBuf::from(fsutil::expand_tilde(raw.trim()));

    if !usable_org_dir(&path) {
        tracing::error!(
            target: "bridge::cowork",
            path = %path.display(),
            subdir = paths::COWORK_PLUGINS_SUBDIR,
            "cowork.session_org_dir is configured but has no plugins subdir; \
             refusing to guess another session"
        );
        return None;
    }

    Some(CoworkTarget {
        cowork_plugins_dir: path.join(paths::COWORK_PLUGINS_SUBDIR),
        session_org_dir: path,
    })
}

fn org_uuid_of(p: &std::path::Path) -> Option<String> {
    p.file_name()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
}

// A half-initialised org dir lacking its `cowork_plugins` subdir would win on
// mtime and route writes nowhere Cowork reads.
fn usable_org_dir(p: &std::path::Path) -> bool {
    p.join(paths::COWORK_PLUGINS_SUBDIR).is_dir()
}

#[must_use]
pub fn pick_target(candidates: &[(SystemTime, PathBuf)]) -> Option<PathBuf> {
    if candidates.is_empty() {
        return None;
    }

    if let Some((_, path)) = candidates.iter().find(|(_, p)| {
        org_uuid_of(p.as_path()).as_deref() == Some(PERSONAL_SESSION_UUID)
            && usable_org_dir(p.as_path())
    }) {
        return Some(path.clone());
    }

    let usable: Vec<&PathBuf> = candidates
        .iter()
        .map(|(_, p)| p)
        .filter(|p| usable_org_dir(p.as_path()))
        .collect();

    match usable.as_slice() {
        [] => {
            tracing::warn!(
                target: "bridge::cowork",
                candidates = candidates.len(),
                "resolve_target: no org dir carries a plugins subdir"
            );
            None
        },
        [only] => Some((*only).clone()),
        many => {
            let listed = many
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            tracing::error!(
                target: "bridge::cowork",
                candidates = %listed,
                "resolve_target: several Cowork org sessions are usable and none is the \
                 personal session; set cowork.session_org_dir to choose one"
            );
            None
        },
    }
}

pub fn apply_enable(target: &CoworkTarget, plugin_ids: &[&str]) -> Result<EmitReport, EmitError> {
    purge_legacy_marketplace(target, LEGACY_SYNTHETIC_PLUGIN, LEGACY_MARKETPLACE_TO_PURGE)?;
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
    purge_legacy_marketplace(target, LEGACY_SYNTHETIC_PLUGIN, LEGACY_MARKETPLACE_TO_PURGE)?;
    Ok(())
}

/// Missing paths are not an error — a no-op on a session that carries no legacy
/// marketplace state.
pub(super) fn purge_legacy_marketplace(
    target: &CoworkTarget,
    plugin_name: &str,
    marketplace: &str,
) -> Result<(), EmitError> {
    let removed_mp_dir = remove_tree(
        &target
            .cowork_plugins_dir
            .join("marketplaces")
            .join(marketplace),
    )?;
    let removed_cache_dir =
        remove_tree(&target.cowork_plugins_dir.join("cache").join(marketplace))?;
    let installed_key = format!("{plugin_name}@{marketplace}");
    let removed_installed = strip_nested_object_key(
        &target.cowork_plugins_dir.join(INSTALLED_PLUGINS_FILE),
        "plugins",
        &installed_key,
    )?;
    let removed_known = strip_top_level_key(
        &target.cowork_plugins_dir.join(KNOWN_MARKETPLACES_FILE),
        marketplace,
    )?;
    clear_enabled(target, plugin_name, marketplace)?;

    if removed_mp_dir || removed_cache_dir || removed_installed || removed_known {
        tracing::info!(
            target: "bridge::cowork",
            marketplace,
            plugin = plugin_name,
            removed_mp_dir,
            removed_cache_dir,
            removed_installed,
            removed_known,
            "purged legacy session marketplace state"
        );
    }
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

fn strip_top_level_key(path: &std::path::Path, key: &str) -> Result<bool, EmitError> {
    let Some(text) = fsutil::read_optional(path).map_err(|e| EmitError::Io {
        context: format!("read {}", path.display()),
        source: e,
    })?
    else {
        return Ok(false);
    };
    let mut root: Value = serde_json::from_str(&text).map_err(CoworkPluginsError::from)?;
    let Some(obj) = root.as_object_mut() else {
        return Ok(false);
    };
    if obj.remove(key).is_none() {
        return Ok(false);
    }
    let bytes = serde_json::to_vec_pretty(&root).map_err(CoworkPluginsError::from)?;
    fsutil::atomic_write_0600(path, &bytes).map_err(|e| EmitError::Io {
        context: format!("atomic_write {}", path.display()),
        source: e,
    })?;
    Ok(true)
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
