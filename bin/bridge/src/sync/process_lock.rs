//! Cross-process single flight for sync.
//!
//! `BridgeContext::sync_lock` serialises syncs inside one process, but a sync
//! also runs from processes that share no memory with the GUI: the OS
//! scheduler's `sync` task and `sync --watch`. Every sync and registry refresh
//! therefore also holds an exclusive lock on `sync.lock` in the bridge metadata
//! directory, so two processes never apply plugins, write host policy or
//! rewrite the sentinel at the same time. A second process waits for the first
//! and then syncs against what it left.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs::File;
use std::path::PathBuf;

use super::SyncError;
use crate::config::paths;

const LOCK_FILE: &str = "sync.lock";

/// Held for the length of one sync; the OS releases the lock when it drops.
#[derive(Debug)]
pub(super) struct ProcessLock {
    _file: File,
}

pub(super) async fn acquire() -> Result<ProcessLock, SyncError> {
    let path = lock_path()?;
    let waited = path.clone();
    tokio::task::spawn_blocking(move || open_and_lock(&path))
        .await
        .map_err(|join| persistence(waited, std::io::Error::other(join)))?
}

fn lock_path() -> Result<PathBuf, SyncError> {
    let dir = paths::bridge_metadata_dir().ok_or_else(|| {
        persistence(
            PathBuf::from(LOCK_FILE),
            std::io::Error::other("no bridge metadata directory is resolvable"),
        )
    })?;
    crate::fsutil::create_dir_all_mode_0700(&dir).map_err(|e| persistence(dir.clone(), e))?;
    Ok(dir.join(LOCK_FILE))
}

fn open_and_lock(path: &std::path::Path) -> Result<ProcessLock, SyncError> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| persistence(path.to_path_buf(), e))?;
    match file.try_lock() {
        Ok(()) => {},
        Err(std::fs::TryLockError::WouldBlock) => {
            tracing::info!(path = %path.display(), "another bridge process is syncing; waiting for it");
            file.lock()
                .map_err(|e| persistence(path.to_path_buf(), e))?;
        },
        Err(std::fs::TryLockError::Error(e)) => return Err(persistence(path.to_path_buf(), e)),
    }
    Ok(ProcessLock { _file: file })
}

const fn persistence(path: PathBuf, source: std::io::Error) -> SyncError {
    SyncError::Persistence { path, source }
}
