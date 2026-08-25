//! Process liveness and port-responsiveness probes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use std::time::Duration;

pub use super::ps::ProcessInfo;
use super::utils;

const HEALTH_CHECK_TIMEOUT_SECS: u64 = 5;

pub async fn is_service_healthy(port: u16) -> McpDomainResult<bool> {
    is_port_responsive(port).await
}

async fn is_port_responsive(port: u16) -> McpDomainResult<bool> {
    use tokio::net::TcpStream;
    use tokio::time::timeout;

    match timeout(
        Duration::from_secs(HEALTH_CHECK_TIMEOUT_SECS),
        TcpStream::connect(format!("127.0.0.1:{port}")),
    )
    .await
    {
        Ok(Ok(_)) => Ok(true),
        _ => Ok(false),
    }
}

pub fn is_process_running(pid: u32) -> bool {
    utils::process_exists(pid) && !systemprompt_models::subprocess::is_zombie(pid)
}

pub fn get_process_info(pid: u32) -> McpDomainResult<Option<ProcessInfo>> {
    super::ps::process_info(pid)
}
