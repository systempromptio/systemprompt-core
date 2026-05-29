//! systemprompt.io HTTP API server.
//!
//! Hosts the Axum application that fronts every protocol surface — A2A, MCP,
//! OAuth, the marketplace, and the AI gateway — wiring [`routes`] and
//! context-extraction [`services::middleware`] onto the shared `AppContext`.
//! [`ApiServer`] is the process entry point; [`HealthChecker`] reports
//! per-module readiness. Failures surface as the [`error`] types and are
//! mapped to HTTP responses at the route boundary.

pub mod error;
pub mod models;
pub mod routes;
pub mod services;

pub use models::ServerConfig;
pub use services::health::{HealthChecker, HealthSummary, ModuleHealth, ProcessMonitor};
pub use services::middleware::{
    A2AContextMiddleware, ContextExtractor, McpContextMiddleware, PublicContextMiddleware,
    UserOnlyContextMiddleware,
};
pub use services::server::ApiServer;
