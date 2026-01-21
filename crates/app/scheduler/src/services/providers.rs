use async_trait::async_trait;
use systemprompt_traits::{
    ProcessCleanupProvider, ProcessProviderError, ProcessResult,
};

use super::orchestration::ProcessCleanup;

#[async_trait]
impl ProcessCleanupProvider for ProcessCleanup {
    fn process_exists(&self, pid: u32) -> bool {
        ProcessCleanup::process_exists(pid)
    }

    fn check_port(&self, port: u16) -> Option<u32> {
        ProcessCleanup::check_port(port)
    }

    fn kill_process(&self, pid: u32) -> bool {
        ProcessCleanup::kill_process(pid)
    }

    async fn terminate_gracefully(&self, pid: u32, grace_period_ms: u64) -> bool {
        ProcessCleanup::terminate_gracefully(pid, grace_period_ms).await
    }

    async fn wait_for_port_free(&self, port: u16, max_retries: u8, delay_ms: u64) -> ProcessResult<()> {
        ProcessCleanup::wait_for_port_free(port, max_retries, delay_ms)
            .await
            .map_err(|_e| ProcessProviderError::PortTimeout(port))
    }
}
