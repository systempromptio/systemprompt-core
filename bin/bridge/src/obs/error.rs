//! Why logging could not be set up: the subscriber itself, or only the log
//! file the process can run without.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

/// The tracing subscriber could not be installed; the process stops.
#[derive(Debug, thiserror::Error)]
pub enum LoggingInitError {
    #[error("RUST_LOG: {0}")]
    Filter(#[source] tracing_subscriber::filter::FromEnvError),
    #[error("initialize tracing: {0}")]
    Subscriber(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

/// The log file could not be opened; logging continues on stderr.
#[derive(Debug, thiserror::Error)]
pub enum LogFileError {
    #[error("cannot resolve bridge log directory")]
    NoDirectory,
    #[error("create log directory {}: {source}", path.display())]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("open log directory {}: {source}", path.display())]
    Open {
        path: PathBuf,
        #[source]
        source: tracing_appender::rolling::InitError,
    },
    #[error("logging worker already installed")]
    WorkerInstalled,
    #[error("logging writer already installed")]
    WriterInstalled,
}
