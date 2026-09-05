//! Unit tests for API services
//!
//! Tests cover:
//! - Health check types (HealthSummary, ModuleHealth)
//! - HealthChecker configuration

mod gateway;
mod health;
mod proxy_audit;
mod proxy_oauth_challenge;
mod request_base_url;
mod server_health_stats;
mod server_reconciliation_stale;
mod validation;
mod analytics_detection;
mod proxy_resolver;
mod server_metrics;
mod server_reconciliation_verify;
mod server_shutdown;
