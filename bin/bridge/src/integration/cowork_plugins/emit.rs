//! IO layer for the Cowork desktop integration.
//!
//! Cowork's filesystem plugin scanner (`NF()` in `app.asar`) reads
//! `%ProgramFiles%\Claude\org-plugins\<plugin>\` on Windows (and the equivalent
//! per-OS path resolved by [`crate::config::paths::org_plugins_effective`]) and
//! attributes every plugin it finds there to the hard-coded
//! `org-provisioned` marketplace. The bridge therefore owes Cowork exactly one
//! write: setting `enabledPlugins["<plugin>@org-provisioned"] = true` in the
//! per-session `cowork_settings.json` so the auto-installed plugin is enabled
//! on first session load.
//!
//! Older bridge builds also wrote a custom `systemprompt-bridge-managed`
//! marketplace tree under `cowork_plugins/marketplaces/`; that surface is gone
//! — the filesystem org-plugin path is the single source of truth.

use std::path::PathBuf;
use std::time::SystemTime;
use std::{fs, io};

use crate::config::paths;

// Cowork creates its "personal" org-session dir under a fixed v4-shaped UUID
// (version=4, variant=DCE, otherwise all zeros with a trailing 1). NOT the
// nil UUID — that's a different sentinel Cowork doesn't use here.
//
// Source of truth in Cowork's app.asar:
//   const ehr = "00000000-0000-4000-8000-000000000001"
//
// If a future Cowork release bumps this string, `pick_target` will silently
// fall through to its mtime fallback. `doctor`'s
// `personal-session sentinel` check is the early-warning that this happened.
pub const PERSONAL_SESSION_UUID: &str = "00000000-0000-4000-8000-000000000001";

// Cowork hard-codes this marketplace identifier for plugins discovered under
// the filesystem org-plugins root; see `NF()` in Claude Cowork's `app.asar`.
pub(super) const ORG_PROVISIONED_MARKETPLACE: &str = "org-provisioned";

// Earlier bridge builds (before the filesystem-scan architecture) published
// the synthetic plugin under this custom marketplace name in
// `cowork_plugins/marketplaces/` + `cowork_plugins/cache/`. Cowork's
// SkillsPlugin loader still prefers session-marketplace state over the
// org-provisioned filesystem scan, so leftover entries here SHADOW the fresh
// content in `%ProgramFiles%\Claude\org-plugins\` and the user sees stale
// skills. Every `apply_enable` actively purges this legacy state.
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

// `None` means no Cowork install detected — callers must treat as no-op,
// not as an error (Cowork is optional).
//
// Resolution order: (1) the personal-session UUID `00000000-…`, (2) fall
// back to the newest-mtime org dir. Mtime alone is not reliable — any Cowork
// interaction with a non-target org bumps its mtime past the one the user
// actually has the Connectors panel pointed at — but personal mode is the
// only mode this gateway can attest to under the 3P spec, so it's the
// preferred target whenever Cowork has materialised it.
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

// An org dir is only usable if its `cowork_plugins` subdir exists; a half-
// initialised org dir would otherwise win on mtime and silently route writes
// nowhere Cowork actually reads.
fn usable_org_dir(p: &std::path::Path) -> bool {
    p.join(paths::COWORK_PLUGINS_SUBDIR).is_dir()
}

// Why: extracted as a pure helper so the resolution rules can be unit-tested
// without staging a real sessions tree on disk. The IO (read_dir) is in
// `resolve_target`; the *choice* between candidates is here.
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

// Removes any artefacts an earlier bridge build wrote under the named
// marketplace: the marketplace + cache dirs, its row in
// `installed_plugins.json` (keyed `<plugin>@<marketplace>`), its entry in
// `known_marketplaces.json`, and the matching enable key in
// `cowork_settings.json`. Missing files/dirs are not an error — this runs on
// every sync and is a no-op on a clean session.
pub(super) fn purge_legacy_marketplace(
    target: &CoworkTarget,
    plugin_name: &str,
    marketplace: &str,
) -> Result<(), EmitError> {
    let removed_mp_dir = remove_tree(&target.cowork_plugins_dir.join("marketplaces").join(marketplace))?;
    let removed_cache_dir = remove_tree(&target.cowork_plugins_dir.join("cache").join(marketplace))?;
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
