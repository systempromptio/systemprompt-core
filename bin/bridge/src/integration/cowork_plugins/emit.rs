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
const PERSONAL_SESSION_UUID: &str = "00000000-0000-4000-8000-000000000001";

// Cowork hard-codes this marketplace identifier for plugins discovered under
// the filesystem org-plugins root; see `NF()` in Claude Cowork's `app.asar`.
pub(super) const ORG_PROVISIONED_MARKETPLACE: &str = "org-provisioned";

use super::CoworkPluginsError;
use super::upsert::{clear_enabled, upsert_enabled};

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
    Ok(())
}
