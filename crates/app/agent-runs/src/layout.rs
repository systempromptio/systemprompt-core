//! [`RunLayout`]: the private directory tree of one run.
//!
//! A run directory holds `home/` (the host's `HOME`, carrying its config and
//! credentials), `workspace/` (the working tree), `logs/` (raw stdout and
//! stderr) and `host/` (generated host config such as the hooks file).
//! `home/` is always deleted after the run, success or not, so no credential
//! outlives it; the rest stays for inspection until the run row is purged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use crate::error::{AgentRunsError, AgentRunsResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLayout {
    pub root: PathBuf,
    pub home: PathBuf,
    pub workspace: PathBuf,
    pub logs: PathBuf,
    pub host: PathBuf,
    pub outputs: PathBuf,
}

impl RunLayout {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            home: root.join("home"),
            workspace: root.join("workspace"),
            logs: root.join("logs"),
            host: root.join("host"),
            outputs: root.join("outputs"),
            root,
        }
    }

    pub async fn create(&self) -> AgentRunsResult<()> {
        for dir in [&self.home, &self.workspace, &self.logs, &self.host] {
            tokio::fs::create_dir_all(dir)
                .await
                .map_err(|source| layout_error(dir, source))?;
        }
        Ok(())
    }

    pub async fn remove_home(&self) -> AgentRunsResult<()> {
        match tokio::fs::remove_dir_all(&self.home).await {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(layout_error(&self.home, source)),
        }
    }
}

fn layout_error(path: &Path, source: std::io::Error) -> AgentRunsError {
    AgentRunsError::Layout {
        path: path.to_path_buf(),
        source,
    }
}
