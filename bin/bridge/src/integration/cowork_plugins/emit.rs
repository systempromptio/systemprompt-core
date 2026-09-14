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

use crate::cowork_compat::PERSONAL_SESSION_UUID;

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

/// Why the Cowork session directory could not be resolved: the machine has
/// no session tree (Cowork never opened), or the tree or the configured
/// override exists but could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ResolveTargetError {
    #[error(transparent)]
    Config(#[from] crate::config::ConfigReadError),
    #[error(
        "cowork.session_org_dir {path} is configured but has no {subdir} subdir; refusing to \
         guess another session"
    )]
    ConfiguredUnusable { path: PathBuf, subdir: &'static str },
    #[error("enumerate {path}: {source}")]
    Enumerate {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "several Cowork org sessions are usable and none is the personal session; set \
         cowork.session_org_dir to choose one"
    )]
    Ambiguous,
}

pub fn resolve_target() -> Result<Option<CoworkTarget>, ResolveTargetError> {
    if let Some(configured) = configured_session_org_dir()? {
        return Ok(Some(configured));
    }

    let Some(sessions_root) = paths::cowork3p_sessions_root() else {
        return Ok(None);
    };
    let sessions = match fs::read_dir(&sessions_root) {
        Ok(sessions) => sessions,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ResolveTargetError::Enumerate {
                path: sessions_root,
                source,
            });
        },
    };
    let mut candidates: Vec<(SystemTime, PathBuf)> = Vec::new();
    for session in sessions {
        let session = session.map_err(|source| ResolveTargetError::Enumerate {
            path: sessions_root.clone(),
            source,
        })?;
        if !session.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let orgs =
            fs::read_dir(session.path()).map_err(|source| ResolveTargetError::Enumerate {
                path: session.path(),
                source,
            })?;
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

    let Some(session_org_dir) = pick_target(&candidates)? else {
        return Ok(None);
    };
    let cowork_plugins_dir = session_org_dir.join(paths::COWORK_PLUGINS_SUBDIR);
    Ok(Some(CoworkTarget {
        session_org_dir,
        cowork_plugins_dir,
    }))
}

fn configured_session_org_dir() -> Result<Option<CoworkTarget>, ResolveTargetError> {
    let cfg = crate::config::Config::load()?;
    let Some(raw) = cfg.cowork.and_then(|c| c.session_org_dir) else {
        return Ok(None);
    };
    let path = PathBuf::from(fsutil::expand_tilde(raw.trim()));

    if !usable_org_dir(&path) {
        return Err(ResolveTargetError::ConfiguredUnusable {
            path,
            subdir: paths::COWORK_PLUGINS_SUBDIR,
        });
    }

    Ok(Some(CoworkTarget {
        cowork_plugins_dir: path.join(paths::COWORK_PLUGINS_SUBDIR),
        session_org_dir: path,
    }))
}

fn org_uuid_of(p: &std::path::Path) -> Option<String> {
    p.file_name()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
}

fn usable_org_dir(p: &std::path::Path) -> bool {
    p.join(paths::COWORK_PLUGINS_SUBDIR).is_dir()
}

pub fn pick_target(
    candidates: &[(SystemTime, PathBuf)],
) -> Result<Option<PathBuf>, ResolveTargetError> {
    if candidates.is_empty() {
        return Ok(None);
    }

    if let Some((_, path)) = candidates.iter().find(|(_, p)| {
        org_uuid_of(p.as_path()).as_deref() == Some(PERSONAL_SESSION_UUID)
            && usable_org_dir(p.as_path())
    }) {
        return Ok(Some(path.clone()));
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
            Ok(None)
        },
        [only] => Ok(Some((*only).clone())),
        many => {
            let listed = many
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            tracing::error!(
                target: "bridge::cowork",
                candidates = %listed,
                "resolve_target: several Cowork org sessions are usable and none is the personal session"
            );
            Err(ResolveTargetError::Ambiguous)
        },
    }
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
