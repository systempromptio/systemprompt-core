//! Failures while formatting a file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::PathBuf;
use std::time::Duration;

/// Failures while formatting a file.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("could not read {} around formatting", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("formatter {name} could not be started from {}", program.display())]
    Spawn {
        name: String,
        program: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("formatter {name} could not be awaited")]
    Wait {
        name: String,
        #[source]
        source: io::Error,
    },
    #[error("formatter {name} timed out after {after:?}")]
    Timeout { name: String, after: Duration },
    #[error("formatter {name} exited unsuccessfully (code {code:?})")]
    Failed {
        name: String,
        code: Option<i32>,
        stderr: String,
    },
}
