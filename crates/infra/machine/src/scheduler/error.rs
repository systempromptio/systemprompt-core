//! Why a scheduler entry could not be described or registered.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::{Path, PathBuf};

/// Why a description is not a valid scheduler entry.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EntryError {
    #[error(
        "{value:?} is not an entry id: letters, digits, '.', '-' and '_', starting with a letter or digit"
    )]
    Id { value: String },
    #[error("an interval of {minutes} minutes is outside one minute to 31 days")]
    Interval { minutes: u32 },
    #[error("an entry needs a one-line description of plain text")]
    Description,
    #[error("{} is not an absolute path of plain text", path.display())]
    Program { path: PathBuf },
    #[error("argument {index} holds a control character")]
    Argument { index: usize },
}

/// Why the system's scheduler could not be read or changed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SchedulerError {
    #[error("resolve the home directory of the current user: {source}")]
    Home {
        #[source]
        source: io::Error,
    },
    #[error("{op} {}: {source}", path.display())]
    Io {
        op: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("start {command}: {source}")]
    Spawn {
        command: String,
        #[source]
        source: io::Error,
    },
    #[error("{command} exited with {code:?}: {stderr}")]
    Exited {
        command: String,
        code: Option<i32>,
        stderr: String,
    },
}

impl SchedulerError {
    pub(crate) fn io(op: &'static str, path: &Path, source: io::Error) -> Self {
        Self::Io {
            op,
            path: path.to_path_buf(),
            source,
        }
    }
}
