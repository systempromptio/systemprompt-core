//! Unit tests for systemprompt-core-api crate
//!
//! Tests cover:
//! - ServerConfig model defaults and construction
//! - JWT token extraction from headers and cookies
//! - Bot detection from user agents
//! - Scanner request detection
//! - Trailing slash redirect logic
//! - Header context extraction
//! - Static content pattern matching
//! - Route types serialization
//! - Health check types (HealthSummary, ModuleHealth)
//! - HealthChecker builder pattern

#[cfg(test)]
mod cowork_audience;
#[cfg(test)]
mod middleware;
#[cfg(test)]
mod models;
#[cfg(test)]
mod routes;
#[cfg(test)]
mod services;
#[cfg(test)]
mod static_content;
