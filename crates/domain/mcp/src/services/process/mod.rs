pub mod cleanup;
pub mod monitor;
pub mod pid_manager;
pub mod spawner;
pub mod utils;

use crate::McpServerConfig;
use anyhow::Result;

#[derive(Debug, Clone, Default)]
pub struct ProcessManager;

impl ProcessManager {
    pub fn new() -> Self {
        Self
    }

    pub fn spawn_server(&self, config: &McpServerConfig) -> Result<u32> {
        spawner::spawn_server(config)
    }

    pub fn is_running(pid: u32) -> bool {
        monitor::is_process_running(pid)
    }

    pub fn find_pid_by_port(port: u16) -> Result<Option<u32>> {
        pid_manager::find_pid_by_port(port)
    }

    pub fn find_process_on_port_with_name(port: u16, name: &str) -> Result<Option<u32>> {
        pid_manager::find_process_on_port_with_name(port, name)
    }

    pub fn verify_binary(config: &McpServerConfig) -> Result<()> {
        spawner::verify_binary(config)
    }

    pub fn build_server(config: &McpServerConfig) -> Result<()> {
        spawner::build_server(config)
    }

    pub fn terminate_gracefully(pid: u32) -> Result<()> {
        cleanup::terminate_gracefully(pid)
    }

    pub fn force_kill(pid: u32) -> Result<()> {
        cleanup::force_kill(pid)
    }
}
