pub mod cleanup;
pub mod monitor;
pub mod pid_manager;
pub mod spawner;

use crate::McpServerConfig;
use anyhow::Result;
use std::sync::Arc;
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone)]
pub struct ProcessManager {
    app_context: Arc<AppContext>,
}

impl ProcessManager {
    pub const fn new(app_context: Arc<AppContext>) -> Self {
        Self { app_context }
    }

    pub fn spawn_server(&self, config: &McpServerConfig) -> Result<u32> {
        spawner::spawn_server(self, config)
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

    pub const fn app_context(&self) -> &Arc<AppContext> {
        &self.app_context
    }
}
