//! Publicly re-exported submodule. See submodule rustdoc for details.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod auth;
pub mod client;
pub mod database;
pub mod deployment;
pub mod lifecycle;
pub mod monitoring;
pub mod network;
pub mod orchestrator;
pub mod process;
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
