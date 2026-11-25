//! Network plumbing for MCP servers.
//!
//! Port allocation and release for MCP server processes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod port;

use crate::error::McpDomainResult;
pub use port::PortHolderClaim;

#[derive(Debug, Clone, Copy)]
pub struct NetworkService;

impl Default for NetworkService {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkService {
    pub const fn new() -> Self {
        Self
    }

    pub async fn prepare_port(
        &self,
        port: u16,
        holder: &PortHolderClaim<'_>,
    ) -> McpDomainResult<()> {
        port::prepare_port(port, holder).await
    }

    pub async fn is_port_responsive(port: u16) -> bool {
        port::is_port_responsive(port).await
    }

    pub async fn wait_for_port_release(&self, port: u16) -> McpDomainResult<()> {
        port::wait_for_port_release(port).await
    }

    pub async fn wait_for_port_release_with_retry(
        &self,
        port: u16,
        holder: &PortHolderClaim<'_>,
        max_attempts: u32,
    ) -> McpDomainResult<()> {
        port::wait_for_port_release_with_retry(port, holder, max_attempts).await
    }
}
