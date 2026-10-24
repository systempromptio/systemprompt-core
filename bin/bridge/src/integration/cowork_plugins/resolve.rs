//! Which Cowork org session the bridge writes into.
//!
//! Precedence: the explicit local pin (`cowork.session_org_dir`), then the
//! session whose directory names the `organization_uuid` the gateway's bridge
//! profile returns, then the personal session or the only usable one.
//! Anything else is ambiguous and is refused, with the candidates attached so
//! the GUI can offer a picker. The bridge never derives the organisation id
//! itself; the gateway is its one source, and [`TargetSource`] says which
//! rule won so the GUI can show it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::{fs, io};

use crate::config::paths;
use crate::cowork_compat::PERSONAL_SESSION_UUID;
use crate::fsutil;

use super::emit::CoworkTarget;

const MIN_ORG_PREFIX: usize = 8;

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
        "several Cowork org sessions are usable and none is the personal session or the one \
         the gateway names; choose one in the bridge (Claude Desktop › Choose session) or set \
         cowork.session_org_dir"
    )]
    Ambiguous { candidates: Vec<PathBuf> },
}

/// Which rule chose the Cowork session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetSource {
    Pinned,
    Gateway,
    Personal,
    OnlySession,
}

impl TargetSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pinned => "pinned",
            Self::Gateway => "gateway",
            Self::Personal => "personal",
            Self::OnlySession => "only-session",
        }
    }
}

pub fn resolve_target() -> Result<Option<CoworkTarget>, ResolveTargetError> {
    Ok(resolve_target_with_source()?.map(|(target, _)| target))
}

pub fn resolve_target_with_source()
-> Result<Option<(CoworkTarget, TargetSource)>, ResolveTargetError> {
    if let Some(configured) = configured_session_org_dir()? {
        return Ok(Some((configured, TargetSource::Pinned)));
    }
    let Some(candidates) = enumerate_candidates()? else {
        return Ok(None);
    };
    let org = gateway_org_id();
    Ok(choose_target(None, org.as_deref(), &candidates)?
        .map(|(path, source)| (target_for(path), source)))
}

pub fn choose_target(
    pinned: Option<PathBuf>,
    org: Option<&str>,
    candidates: &[(SystemTime, PathBuf)],
) -> Result<Option<(PathBuf, TargetSource)>, ResolveTargetError> {
    if let Some(path) = pinned {
        return Ok(Some((path, TargetSource::Pinned)));
    }
    if let Some(path) = org.and_then(|org| match_org(candidates, org)) {
        return Ok(Some((path, TargetSource::Gateway)));
    }
    pick_target(candidates)
}

pub fn enumerate_candidates() -> Result<Option<Vec<(SystemTime, PathBuf)>>, ResolveTargetError> {
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
    Ok(Some(candidates))
}

fn target_for(session_org_dir: PathBuf) -> CoworkTarget {
    CoworkTarget {
        cowork_plugins_dir: session_org_dir.join(paths::COWORK_PLUGINS_SUBDIR),
        session_org_dir,
    }
}

fn gateway_org_id() -> Option<String> {
    match crate::auth::profile_cache::load() {
        Ok(profile) => profile.and_then(|p| p.organization_uuid),
        Err(e) => {
            tracing::debug!(
                target: "bridge::cowork",
                error = %e,
                "no cached profile for the org id"
            );
            None
        },
    }
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

    Ok(Some(target_for(path)))
}

fn org_uuid_of(p: &Path) -> Option<String> {
    p.file_name()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
}

fn usable_org_dir(p: &Path) -> bool {
    p.join(paths::COWORK_PLUGINS_SUBDIR).is_dir()
}

fn normalized(id: &str) -> String {
    id.chars()
        .filter(|c| *c != '-')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[must_use]
pub fn dir_names_org(dir_name: &str, org: &str) -> bool {
    let dir = normalized(dir_name);
    let org = normalized(org);
    let shorter = dir.len().min(org.len());
    shorter >= MIN_ORG_PREFIX && dir.get(..shorter) == org.get(..shorter)
}

#[must_use]
pub fn match_org(candidates: &[(SystemTime, PathBuf)], org: &str) -> Option<PathBuf> {
    let mut matching = candidates.iter().map(|(_, p)| p).filter(|p| {
        usable_org_dir(p) && org_uuid_of(p).is_some_and(|name| dir_names_org(&name, org))
    });
    let first = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    Some(first.clone())
}

pub fn pick_target(
    candidates: &[(SystemTime, PathBuf)],
) -> Result<Option<(PathBuf, TargetSource)>, ResolveTargetError> {
    if candidates.is_empty() {
        return Ok(None);
    }

    if let Some((_, path)) = candidates.iter().find(|(_, p)| {
        org_uuid_of(p.as_path()).as_deref() == Some(PERSONAL_SESSION_UUID)
            && usable_org_dir(p.as_path())
    }) {
        return Ok(Some((path.clone(), TargetSource::Personal)));
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
        [only] => Ok(Some(((*only).clone(), TargetSource::OnlySession))),
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
            Err(ResolveTargetError::Ambiguous {
                candidates: many.iter().map(|p| (*p).clone()).collect(),
            })
        },
    }
}
