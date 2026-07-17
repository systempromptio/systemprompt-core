//! systemprompt.io HTTP API server.
//!
//! Hosts the Axum application that fronts every protocol surface — A2A, MCP,
//! OAuth, the marketplace, and the AI gateway — wiring [`routes`] and
//! context-extraction [`services::middleware`] onto the shared `AppContext`.
//! [`services::server::bind_and_serve`] binds the listener before bootstrap
//! and [`services::server::run_server`] swaps in the full router;
//! [`HealthChecker`] reports per-module readiness. Failures surface as the
//! [`error`] types and are mapped to HTTP responses at the route boundary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod error;
pub mod routes;
pub mod services;

pub use services::health::{HealthChecker, HealthSummary, ModuleHealth, ProcessMonitor};
pub use services::middleware::{
    A2AContextMiddleware, ContextExtractor, McpContextMiddleware, PublicContextMiddleware,
    UserOnlyContextMiddleware,
};
