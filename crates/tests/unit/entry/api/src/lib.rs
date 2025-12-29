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

mod middleware;
mod models;
mod routes;
mod services;
mod static_content;
