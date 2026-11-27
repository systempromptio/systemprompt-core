//! Failures of the snapshot store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::{Path, PathBuf};

use harness_tasks::TaskError;

use super::SnapshotId;

/// Failures of the snapshot store.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error("snapshot i/o failed at {}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("snapshot manifest {} could not be encoded or decoded", path.display())]
    Manifest {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("snapshot {0} does not exist")]
    NotFound(SnapshotId),
    #[error("not a snapshot id")]
    InvalidId,
    #[error("object {hash} is missing from the snapshot store")]
    MissingObject { hash: String },
    #[error("object {hash} in the snapshot store is corrupt")]
    CorruptObject { hash: String },
    #[error("the worktree has more than {limit} files to snapshot")]
    TooManyFiles { limit: usize },
    #[error("{} is outside the worktree", path.display())]
    OutsideWorktree { path: PathBuf },
    #[error("snapshot task did not run to completion")]
    Task(#[source] TaskError),
}

pub(crate) fn io_at(path: &Path) -> impl FnOnce(io::Error) -> SnapshotError + '_ {
    move |source| SnapshotError::Io {
        path: path.to_path_buf(),
        source,
    }
}
