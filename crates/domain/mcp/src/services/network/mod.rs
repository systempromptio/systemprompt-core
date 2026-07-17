//! Network plumbing for MCP servers.
//!
//! Port allocation and release, the base Axum router with CORS, and
//! reverse-proxy routers to upstream services.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod port;
pub mod proxy;
pub mod routing;

use crate::error::McpDomainResult;

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

    pub async fn prepare_port(&self, port: u16) -> McpDomainResult<()> {
        port::prepare_port(port).await
    }

    pub fn is_port_responsive(port: u16) -> bool {
        port::is_port_responsive(port)
    }

    pub async fn wait_for_port_release(&self, port: u16) -> McpDomainResult<()> {
        port::wait_for_port_release(port).await
    }

    pub async fn wait_for_port_release_with_retry(
        &self,
        port: u16,
        max_attempts: u32,
    ) -> McpDomainResult<()> {
        port::wait_for_port_release_with_retry(port, max_attempts).await
    }

    pub const fn cleanup_port_resources(port: u16) {
        port::cleanup_port_resources(port);
    }

    pub fn create_router() -> axum::Router {
        routing::create_base_router()
    }

    pub fn apply_cors(router: axum::Router) -> McpDomainResult<axum::Router> {
        routing::apply_cors_layer(router)
    }

    pub fn create_proxy(target_host: &str, target_port: u16) -> axum::Router {
        proxy::create_proxy_router(target_host, target_port)
    }
}
