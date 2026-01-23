use async_trait::async_trait;
use std::sync::Arc;

pub type ProcessResult<T> = Result<T, ProcessProviderError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProcessProviderError {
    #[error("Process not found: {0}")]
    NotFound(u32),

    #[error("Operation failed: {0}")]
    OperationFailed(String),

    #[error("Timeout waiting for port {0}")]
    PortTimeout(u16),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for ProcessProviderError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

#[async_trait]
pub trait ProcessCleanupProvider: Send + Sync {
    fn process_exists(&self, pid: u32) -> bool;

    fn check_port(&self, port: u16) -> Option<u32>;

    fn kill_process(&self, pid: u32) -> bool;

    async fn terminate_gracefully(&self, pid: u32, grace_period_ms: u64) -> bool;

    async fn wait_for_port_free(
        &self,
        port: u16,
        max_retries: u8,
        delay_ms: u64,
    ) -> ProcessResult<()>;
}

pub type DynProcessCleanupProvider = Arc<dyn ProcessCleanupProvider>;
