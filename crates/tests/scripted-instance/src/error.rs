//! The crate's error type.

use std::path::PathBuf;
use std::time::Duration;

use crate::event::Event;

#[derive(Debug, thiserror::Error)]
pub enum ScriptedInstanceError {
    #[error("failed to bind a loopback listener on port {port}")]
    Bind {
        port: u16,
        #[source]
        source: std::io::Error,
    },

    #[error("the scripted instance server failed")]
    Serve(#[source] std::io::Error),

    #[error("the scripted instance server task did not finish")]
    Join(#[from] tokio::task::JoinError),

    #[error("failed to read {}", .path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse {}", .path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to load or write the key file {}", .path.display())]
    KeyFile {
        path: PathBuf,
        #[source]
        source: systemprompt_device::secure::SecureError,
    },

    #[error("the key file {} holds {found} bytes, expected {expected}", .path.display())]
    KeyFileMalformed {
        path: PathBuf,
        expected: usize,
        found: usize,
    },

    #[error("failed to parse inline scenario JSON")]
    ParseInline(#[source] serde_json::Error),

    #[error("{what} is not implemented")]
    NotImplemented { what: &'static str },

    #[error("no {event:?} request arrived within {waited:?}")]
    WaitTimedOut { event: Event, waited: Duration },
}
