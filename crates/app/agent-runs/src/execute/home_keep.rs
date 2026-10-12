//! Carries a host's session store across attempts of one run.
//!
//! `home/` is deleted after every attempt, but a host keeps the transcripts
//! a resume needs under it (Claude Code `.claude/projects`, Codex
//! `.codex/sessions`). Each path the adapter names in
//! `HeadlessHost::persistent_home_paths` is moved to `host/home-keep/` after
//! the exit and moved back into `home/` before the next spawn. Both trees sit
//! under the run root, so a move is a rename.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use crate::error::{HeadlessError, HeadlessResult};
use crate::layout::RunLayout;

const KEEP_DIR: &str = "home-keep";

fn keep_root(layout: &RunLayout) -> PathBuf {
    layout.host.join(KEEP_DIR)
}

pub(super) async fn restore(layout: &RunLayout, paths: &[&str]) -> HeadlessResult<()> {
    for rel in paths {
        let from = keep_root(layout).join(rel);
        if tokio::fs::try_exists(&from)
            .await
            .map_err(|e| err(&from, e))?
        {
            let to = layout.home.join(rel);
            replace(&from, &to).await?;
        }
    }
    Ok(())
}

pub(super) async fn preserve(layout: &RunLayout, paths: &[&str]) -> HeadlessResult<()> {
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

async fn replace(from: &Path, to: &Path) -> HeadlessResult<()> {
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

fn err(path: &Path, source: std::io::Error) -> HeadlessError {
    HeadlessError::Layout {
        path: path.to_path_buf(),
        source,
    }
}
