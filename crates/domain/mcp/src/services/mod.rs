pub mod auth;
pub mod client;
pub mod database;
pub mod deployment;
pub mod lifecycle;
pub mod monitoring;
pub mod network;
pub mod orchestrator;
pub mod process;
mod providers;
pub mod registry;
pub mod schema;
pub mod tool_provider;
pub mod ui_renderer;

pub use database::{DatabaseManager, ServiceInfo};
pub use deployment::DeploymentService;
pub use lifecycle::LifecycleManager;
pub use monitoring::proxy_health::{ProxyHealthCheck, RoutableService};
pub use monitoring::MonitoringManager;
pub use network::NetworkManager;
pub use orchestrator::McpOrchestrator;
pub use process::ProcessManager;
pub use registry::RegistryManager;

pub use orchestrator::{EventBus, McpEvent};
pub use tool_provider::McpToolProvider;

pub use McpOrchestrator as McpManager;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ServiceManager {
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn restart(&self) -> Result<()>;
    async fn status(&self) -> Result<String>;
}

#[async_trait]
pub trait ServiceLifecycle {
    async fn initialize(&mut self) -> Result<()>;
    async fn shutdown(&mut self) -> Result<()>;
    async fn health_check(&self) -> Result<bool>;
}
