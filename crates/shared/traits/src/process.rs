//! Process and port management provider trait.

use async_trait::async_trait;
use std::sync::Arc;

/// Result alias for [`ProcessCleanupProvider`] operations.
pub type ProcessResult<T> = Result<T, ProcessProviderError>;

/// Errors returned by process management providers.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProcessProviderError {
    /// The requested PID does not exist on the host.
    #[error("Process not found: {0}")]
    NotFound(u32),

    /// A syscall or higher-level operation failed.
    #[error("Operation failed: {0}")]
    OperationFailed(String),

    /// The waited-for port did not free within the allotted retries.
    #[error("Timeout waiting for port {0}")]
    PortTimeout(u16),

    /// Catch-all for unexpected provider failures.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for ProcessProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

/// Inspect and clean up local OS processes during startup.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `Arc<dyn ProcessCleanupProvider>` via [`DynProcessCleanupProvider`].
#[async_trait]
pub trait ProcessCleanupProvider: Send + Sync {
    /// Report whether `pid` resolves to a live process.
    fn process_exists(&self, pid: u32) -> bool;

    /// Return the PID currently bound to `port`, if any.
    fn check_port(&self, port: u16) -> Option<u32>;

    /// Send `SIGKILL` (or platform equivalent) to `pid`.
    fn kill_process(&self, pid: u32) -> bool;

    /// Send a graceful shutdown signal and wait up to `grace_period_ms`
    /// for the process to exit on its own.
    async fn terminate_gracefully(&self, pid: u32, grace_period_ms: u64) -> bool;

    /// Poll up to `max_retries` times for `port` to become free, sleeping
    /// `delay_ms` between attempts.
    async fn wait_for_port_free(
        &self,
        port: u16,
        max_retries: u8,
        delay_ms: u64,
    ) -> ProcessResult<()>;
}

/// Shared `Arc` alias for [`ProcessCleanupProvider`].
pub type DynProcessCleanupProvider = Arc<dyn ProcessCleanupProvider>;
