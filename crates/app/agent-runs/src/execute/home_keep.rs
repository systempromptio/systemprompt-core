//! Carries a host's session store across attempts of one run.
//!
//! `home/` is deleted after every attempt, but a host keeps the transcripts
//! a resume needs under it (Claude Code `.claude/projects`, Codex
//! `.codex/sessions`). Each path the adapter names in
//! `HeadlessHost::persistent_home_paths` is moved to `host/home-keep/` after
//! the exit and moved back into `home/` before the next spawn. Both trees sit
//! under the run root, so a move is a rename. A host that keeps nothing here
//! reports [`SessionStore::NotKept`]; one whose kept store is missing (the
//! run moved to a claimer without the previous run root) reports
//! [`SessionStore::Absent`], and the executor starts that attempt fresh.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::error::{AgentRunsError, AgentRunsResult};
use crate::layout::RunLayout;
use crate::request::RunRequest;

const KEEP_DIR: &str = "home-keep";

fn keep_root(layout: &RunLayout) -> PathBuf {
    layout.host.join(KEEP_DIR)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SessionStore {
    Restored,
    Absent,
    NotKept,
}

pub(super) async fn restore(layout: &RunLayout, paths: &[&str]) -> AgentRunsResult<SessionStore> {
    if paths.is_empty() {
        return Ok(SessionStore::NotKept);
    }
    let mut store = SessionStore::Absent;
    for rel in paths {
        let from = keep_root(layout).join(rel);
        if tokio::fs::try_exists(&from)
            .await
            .map_err(|e| err(&from, e))?
        {
            let to = layout.home.join(rel);
            replace(&from, &to).await?;
            store = SessionStore::Restored;
        }
    }
    Ok(store)
}

pub(super) async fn preserve(layout: &RunLayout, paths: &[&str]) -> AgentRunsResult<()> {
    for rel in paths {
        let from = layout.home.join(rel);
        if tokio::fs::try_exists(&from)
            .await
            .map_err(|e| err(&from, e))?
        {
            let to = keep_root(layout).join(rel);
            replace(&from, &to).await?;
        }
    }
    Ok(())
}

pub(super) fn spawn_request<'a>(
    request: &'a RunRequest,
    layout: &RunLayout,
    store: SessionStore,
) -> Cow<'a, RunRequest> {
    match (&request.resume_session_id, store) {
        (Some(session), SessionStore::Absent) => {
            tracing::warn!(
                run_root = %layout.root.display(),
                attempt = request.attempt,
                dropped_session_id = %session.as_str(),
                "host session store not present on this claimer; starting a fresh session"
            );
            let mut fresh = request.clone();
            fresh.resume_session_id = None;
            Cow::Owned(fresh)
        },
        _ => Cow::Borrowed(request),
    }
}

async fn replace(from: &Path, to: &Path) -> AgentRunsResult<()> {
    match tokio::fs::remove_dir_all(to).await {
        Ok(()) => {},
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) if e.kind() == std::io::ErrorKind::NotADirectory => {
            tokio::fs::remove_file(to).await.map_err(|e| err(to, e))?;
        },
        Err(e) => return Err(err(to, e)),
    }
    if let Some(parent) = to.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| err(parent, e))?;
    }
    tokio::fs::rename(from, to).await.map_err(|e| err(from, e))
}

fn err(path: &Path, source: std::io::Error) -> AgentRunsError {
    AgentRunsError::Layout {
        path: path.to_path_buf(),
        source,
    }
}
