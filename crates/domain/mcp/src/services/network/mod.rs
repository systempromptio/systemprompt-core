pub mod port_manager;
pub mod proxy;
pub mod routing;

use anyhow::Result;

#[derive(Debug, Clone, Copy)]
pub struct NetworkManager;

impl Default for NetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkManager {
    pub const fn new() -> Self {
        Self
    }

    pub async fn prepare_port(&self, port: u16) -> Result<()> {
        port_manager::prepare_port(port).await
    }

    pub fn is_port_responsive(port: u16) -> bool {
        port_manager::is_port_responsive(port)
    }

    pub async fn wait_for_port_release(&self, port: u16) -> Result<()> {
        port_manager::wait_for_port_release(port).await
    }

    pub const fn cleanup_port_resources(port: u16) {
        port_manager::cleanup_port_resources(port);
    }

    pub fn create_router() -> axum::Router {
        routing::create_base_router()
    }

    pub fn apply_cors(router: axum::Router) -> Result<axum::Router> {
        routing::apply_cors_layer(router)
    }

    pub fn create_proxy(target_host: &str, target_port: u16) -> axum::Router {
        proxy::create_proxy_router(target_host, target_port)
    }
}
