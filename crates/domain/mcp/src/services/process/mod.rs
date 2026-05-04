pub mod cleanup;
pub mod monitor;
pub mod pid_manager;
pub mod spawner;
pub mod utils;

use crate::McpServerConfig;
use crate::error::McpDomainResult;
use systemprompt_models::AppPaths;

#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessManager;

impl ProcessManager {
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
        pid_manager::find_pid_by_port(port)
    }

    pub fn find_process_on_port_with_name(port: u16, name: &str) -> McpDomainResult<Option<u32>> {
        pid_manager::find_process_on_port_with_name(port, name)
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
}
