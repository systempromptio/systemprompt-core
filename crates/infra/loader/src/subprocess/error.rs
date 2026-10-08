//! Typed failures of process supervision.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// Why a supervision call could not establish or change a process's state.
#[derive(Debug, thiserror::Error)]
pub enum SupervisionError {
    #[error("pid {pid} is not a signalable process id")]
    NotSignalable { pid: u32 },

    #[error("failed to send {signal} to pid {pid}")]
    Signal {
        pid: u32,
        signal: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error("pid {pid} was still running after a forced kill")]
    Survived { pid: u32 },

    #[error("failed to run `{tool}`")]
    Tool {
        tool: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error("`{tool}` failed with {status}")]
    ToolFailed {
        tool: &'static str,
        status: std::process::ExitStatus,
    },

    #[error("process supervision is not supported on this platform")]
    Unsupported,

    #[error("blocking supervision task did not complete")]
    Join(#[from] tokio::task::JoinError),
}
