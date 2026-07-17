//! OS-process lifecycle for MCP servers: spawning, PID discovery,
//! liveness monitoring, and graceful/forced termination.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod cleanup;
pub mod monitor;
pub mod pid;
pub mod spawner;
pub mod utils;

use crate::McpServerConfig;
use crate::error::McpDomainResult;
use systemprompt_models::AppPaths;

#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessService;

impl ProcessService {
    pub const fn new() -> Self {
        Self
    }

    pub fn spawn_server(paths: &AppPaths, config: &McpServerConfig) -> McpDomainResult<u32> {
        spawner::spawn_server(paths, config)
    }

    pub fn is_running(pid: u32) -> bool {
        monitor::is_process_running(pid)
    }

    pub fn find_pid_by_port(port: u16) -> McpDomainResult<Option<u32>> {
        pid::find_pid_by_port(port)
    }

    pub fn find_process_on_port_with_name(port: u16, name: &str) -> McpDomainResult<Option<u32>> {
        pid::find_process_on_port_with_name(port, name)
    }

    pub fn verify_binary(paths: &AppPaths, config: &McpServerConfig) -> McpDomainResult<()> {
        spawner::verify_binary(paths, config)
    }

    pub fn build_server(config: &McpServerConfig) -> McpDomainResult<()> {
        spawner::build_server(config)
    }

    pub fn terminate_gracefully(pid: u32) -> McpDomainResult<()> {
        cleanup::terminate_gracefully(pid)
    }

    pub fn force_kill(pid: u32) -> McpDomainResult<()> {
        cleanup::force_kill(pid)
    }

    pub async fn terminate_gracefully_verified(
        pid: u32,
        service_name: &str,
    ) -> McpDomainResult<()> {
        cleanup::terminate_gracefully_verified(pid, service_name).await
    }
}
