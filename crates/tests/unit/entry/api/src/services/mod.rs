//! Unit tests for API services
//!
//! Tests cover:
//! - Health check types (HealthSummary, ModuleHealth)
//! - HealthChecker configuration

mod analytics_detection;
mod health;
mod process_ownership;
mod process_rendezvous;
mod proxy_audit;
mod proxy_fixed_arguments;
mod proxy_oauth_challenge;
mod proxy_resolver;
mod request_base_url;
mod server_health_stats;
mod server_metrics;
mod server_ops_metrics;
mod server_reconciliation_verify;
mod server_shutdown;
mod validation;

mod coverage_agent_reconciliation;
