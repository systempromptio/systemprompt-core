//! IO layer for the Cowork desktop integration.
//!
//! Writes `enabledPlugins["<plugin>@org-provisioned"] = true` in the per-session
//! `cowork_settings.json` and purges any legacy session-marketplace state.

use std::path::PathBuf;
use std::time::SystemTime;
use std::{fs, io};

use crate::config::paths;

// Cowork's fixed sentinel for the personal org-session dir; if a future release
// changes it, `pick_target` silently falls back to mtime (doctor's
// `personal-session sentinel` check warns on this).
pub const PERSONAL_SESSION_UUID: &str = "00000000-0000-4000-8000-000000000001";

pub(super) const ORG_PROVISIONED_MARKETPLACE: &str = "org-provisioned";

// Legacy session-marketplace entries shadow the org-provisioned filesystem
// scan, so every `apply_enable` purges them.
const LEGACY_MARKETPLACE_TO_PURGE: &str = "systemprompt-bridge-managed";

const INSTALLED_PLUGINS_FILE: &str = "installed_plugins.json";
const KNOWN_MARKETPLACES_FILE: &str = "known_marketplaces.json";

use super::CoworkPluginsError;
use super::upsert::{clear_enabled, upsert_enabled};
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
/// error. Prefers the personal-session dir, falling back to newest-mtime.
#[must_use]
pub fn resolve_target() -> Option<CoworkTarget> {
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

    let mut by_mtime: Vec<&(SystemTime, PathBuf)> = candidates.iter().collect();
    by_mtime.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    let fallback = by_mtime.into_iter().next().map(|(_, p)| p.clone());
    if let Some(ref path) = fallback {
        tracing::warn!(
            target: "bridge::cowork",
            path = %path.display(),
            "resolve_target: personal-session dir missing; falling back to newest-mtime org dir"
        );
    }
    fallback
}

pub fn apply_enable(target: &CoworkTarget, plugin_name: &str) -> Result<EmitReport, EmitError> {
    purge_legacy_marketplace(target, plugin_name, LEGACY_MARKETPLACE_TO_PURGE)?;
    let enabled = upsert_enabled(target, plugin_name, ORG_PROVISIONED_MARKETPLACE)?;
    tracing::info!(
        target: "bridge::cowork",
        session_org = %target.session_org_dir.display(),
        plugin = plugin_name,
        marketplace = ORG_PROVISIONED_MARKETPLACE,
        "enabled bridge plugin in Cowork settings"
    );
    Ok(EmitReport {
        target: Some(target.session_org_dir.clone()),
        enabled,
    })
}

pub fn clear_all(target: &CoworkTarget, plugin_name: &str) -> Result<(), EmitError> {
    clear_enabled(target, plugin_name, ORG_PROVISIONED_MARKETPLACE)?;
    purge_legacy_marketplace(target, plugin_name, LEGACY_MARKETPLACE_TO_PURGE)?;
    Ok(())
}

/// Removes legacy marketplace artefacts (dirs, `installed_plugins.json` row,
/// `known_marketplaces.json` entry, enable key). Missing paths are not an error;
/// this is a no-op on a clean session.
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

fn remove_tree(path: &std::path::Path) -> Result<bool, EmitError> {
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

fn strip_nested_object_key(
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
