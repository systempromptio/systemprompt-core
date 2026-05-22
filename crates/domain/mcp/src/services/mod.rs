//! Publicly re-exported submodule. See submodule rustdoc for details.

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

pub use database::{DatabaseService, ServiceInfo};
pub use deployment::DeploymentService;
pub use lifecycle::LifecycleOrchestrator;
pub use monitoring::MonitoringService;
pub use monitoring::proxy_health::{ProxyHealthCheck, RoutableService};
pub use network::NetworkService;
pub use orchestrator::McpOrchestrator;
pub use process::ProcessService;
pub use registry::RegistryService;

pub use orchestrator::{EventBus, McpEvent};
pub use tool_provider::McpToolProvider;


use crate::error::McpDomainResult;
use async_trait::async_trait;

#[async_trait]
pub trait ServiceManager {
    async fn start(&self) -> McpDomainResult<()>;
    async fn stop(&self) -> McpDomainResult<()>;
    async fn restart(&self) -> McpDomainResult<()>;
    async fn status(&self) -> McpDomainResult<String>;
}

#[async_trait]
pub trait ServiceLifecycle {
    async fn initialize(&mut self) -> McpDomainResult<()>;
    async fn shutdown(&mut self) -> McpDomainResult<()>;
    async fn health_check(&self) -> McpDomainResult<bool>;
}
