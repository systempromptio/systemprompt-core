//! The GUI's fix for an ambiguous Cowork session: list the usable org
//! sessions and pin the one the person picks as `cowork.session_org_dir`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::paths;

use super::resolve::{ResolveTargetError, enumerate_candidates};

#[derive(Debug, thiserror::Error)]
pub enum ChooseSessionError {
    #[error(transparent)]
    Resolve(#[from] ResolveTargetError),
    #[error("{path} is not one of this computer's Cowork org sessions")]
    NotACandidate { path: PathBuf },
    #[error(transparent)]
    Write(#[from] crate::config::ConfigWriteError),
}

/// One Cowork org session the person can pick, newest first.
#[derive(Debug, Clone, Serialize)]
pub struct CoworkSessionChoice {
    pub path: String,
    pub org: String,
    pub modified_unix: u64,
}

pub fn usable_sessions() -> Result<Vec<CoworkSessionChoice>, ResolveTargetError> {
    let mut candidates = enumerate_candidates()?.unwrap_or_default();
    candidates.retain(|(_, p)| p.join(paths::COWORK_PLUGINS_SUBDIR).is_dir());
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(candidates
        .into_iter()
        .map(|(mtime, path)| CoworkSessionChoice {
            org: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            modified_unix: mtime
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            path: path.display().to_string(),
        })
        .collect())
}

/// Pins `path` as the Cowork session. Only a session [`usable_sessions`]
/// lists is accepted, so the webview cannot point the bridge anywhere else.
pub fn choose_session(path: &Path) -> Result<(), ChooseSessionError> {
    let wanted = path.display().to_string();
    if !usable_sessions()?.iter().any(|c| c.path == wanted) {
        return Err(ChooseSessionError::NotACandidate {
            path: path.to_owned(),
        });
    }
    crate::config::write::edit(|doc| {
        crate::config::write::set(doc, &["cowork", "session_org_dir"], wanted.as_str())
    })?;
    tracing::info!(
        target: "bridge::cowork",
        session_org = %wanted,
        "pinned the Cowork session"
    );
    Ok(())
}
