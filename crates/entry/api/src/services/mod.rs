//! HTTP service layer for the API server.
//!
//! Groups the gateway, proxy, middleware, static-content, and server-lifecycle
//! services that the binary wires together. Re-exports the health-check surface
//! ([`HealthChecker`], [`HealthSummary`], [`ModuleHealth`], [`ProcessMonitor`])
//! used by readiness probes.

pub mod gateway;
pub mod health;
pub mod middleware;
pub mod proxy;
pub mod request_base_url;
pub mod server;
pub mod static_content;
pub mod validation;

pub use health::{HealthChecker, HealthSummary, ModuleHealth, ProcessMonitor};
