//! OS-process lifecycle for MCP servers: spawning, liveness, port holders
//! and stops.
//!
//! Spawning is local ([`spawner`]); everything that probes or signals a live
//! process goes through [`systemprompt_loader::subprocess`], so an MCP server
//! is stopped only when it is provably this installation's child for that
//! service (its recorded pid carries the matching [`ChildKind::Mcp`] marker).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod spawner;

use std::time::Duration;

use crate::McpServerConfig;
use crate::error::McpDomainResult;
use systemprompt_config::paths::AppPaths;
use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::{self, ChildKind, StopOutcome};

const STOP_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessService;

impl ProcessService {
    pub const fn new() -> Self {
        Self
    }

    pub fn spawn_server(paths: &AppPaths, config: &McpServerConfig) -> McpDomainResult<u32> {
        spawner::spawn_server(paths, config)
    }

    pub async fn is_running(pid: u32) -> bool {
        subprocess::is_running(pid).await
    }

    pub async fn owned_port_holders(
        port: u16,
        service_name: &ServiceName,
    ) -> McpDomainResult<Vec<u32>> {
        let mut owned = Vec::new();
        for pid in subprocess::pids_listening_on(port).await? {
            if subprocess::owns(pid, ChildKind::Mcp, service_name).await {
                owned.push(pid);
            }
        }
        Ok(owned)
    }

    pub async fn port_has_listener(port: u16) -> McpDomainResult<bool> {
        Ok(!subprocess::pids_listening_on(port).await?.is_empty())
    }

    pub fn verify_binary(paths: &AppPaths, config: &McpServerConfig) -> McpDomainResult<()> {
        spawner::verify_binary(paths, config)
    }

    pub async fn build_server(config: &McpServerConfig) -> McpDomainResult<()> {
        let config = config.clone();
        tokio::task::spawn_blocking(move || spawner::build_server(&config)).await?
    }

    pub async fn stop(pid: u32, service_name: &ServiceName) -> McpDomainResult<StopOutcome> {
        Ok(subprocess::stop_owned(pid, ChildKind::Mcp, service_name, STOP_GRACE).await?)
    }
}
