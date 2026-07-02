//! Long-lived per-server health monitor.
//!
//! Polls [`perform_health_check`] on a fixed interval and logs degradation and
//! recovery transitions, tracking consecutive failures and downtime between the
//! last failure and recovery.

use chrono::{DateTime, Utc};
use std::time::Duration;
use tokio::time::interval;
use tracing::Instrument;

use super::health::{HealthCheckResult, HealthStatus, perform_health_check};
use crate::McpServerConfig;
use crate::error::McpDomainResult;

#[derive(Debug, Default, Clone, Copy)]
pub struct HealthMonitorState {
    previous_status: Option<HealthStatus>,
    failure_count: u32,
    last_failure_time: Option<DateTime<Utc>>,
}

impl HealthMonitorState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            previous_status: None,
            failure_count: 0,
            last_failure_time: None,
        }
    }

    pub fn observe(&mut self, config: &McpServerConfig, result: &HealthCheckResult) {
        handle_health_result(config, result, self);
        self.previous_status = Some(result.status);
    }

    #[must_use]
    pub const fn failure_count(&self) -> u32 {
        self.failure_count
    }

    #[must_use]
    pub const fn previous_status(&self) -> Option<HealthStatus> {
        self.previous_status
    }
}

pub async fn monitor_health_continuously(
    config: &McpServerConfig,
    report_interval: Duration,
) -> McpDomainResult<()> {
    let span: tracing::Span = systemprompt_logging::SystemSpan::new("mcp_health_monitor").into();
    async move {
        let mut ticker = interval(report_interval);
        let mut state = HealthMonitorState::new();

        loop {
            ticker.tick().await;

            match perform_health_check(config).await {
                Ok(result) => state.observe(config, &result),
                Err(e) => log_health_check_error(config, &e),
            }
        }
    }
    .instrument(span)
    .await
}

fn handle_health_result(
    config: &McpServerConfig,
    result: &HealthCheckResult,
    state: &mut HealthMonitorState,
) {
    match result.status {
        HealthStatus::Unhealthy => handle_unhealthy(config, result, state),
        HealthStatus::Healthy => handle_healthy(config, state),
        HealthStatus::Degraded => handle_degraded(config, result, state),
        HealthStatus::Unknown => {},
    }
}

fn handle_unhealthy(
    config: &McpServerConfig,
    result: &HealthCheckResult,
    state: &mut HealthMonitorState,
) {
    state.failure_count += 1;
    state.last_failure_time = Some(Utc::now());

    if state.previous_status != Some(HealthStatus::Unhealthy) {
        let degradation_reason = get_error_message(result.details.error_message.as_ref());
        tracing::info!(
            service_name = %config.name,
            health_score = result.status.as_str(),
            degradation_reason = degradation_reason,
            impact_level = "high",
            recovery_actions = ?["restart_service", "check_port_availability"],
            "MCP service health degraded"
        );
    }

    let error_msg = get_error_message(result.details.error_message.as_ref());
    tracing::error!(service_name = %config.name, error = error_msg, "Service is unhealthy");
}

fn handle_healthy(config: &McpServerConfig, state: &mut HealthMonitorState) {
    if state.previous_status == Some(HealthStatus::Unhealthy) && state.failure_count > 0 {
        let downtime = state
            .last_failure_time
            .map_or(0, |t| Utc::now().signed_duration_since(t).num_seconds());

        tracing::info!(
            service_name = %config.name,
            downtime_duration = downtime,
            recovery_method = "automatic",
            health_score = "healthy",
            failure_count = state.failure_count,
            "MCP service recovered"
        );

        state.failure_count = 0;
        state.last_failure_time = None;
    }
}

fn handle_degraded(
    config: &McpServerConfig,
    result: &HealthCheckResult,
    state: &HealthMonitorState,
) {
    if state.previous_status == Some(HealthStatus::Healthy) {
        tracing::info!(
            service_name = %config.name,
            latency_ms = result.latency_ms,
            performance_threshold_exceeded = true,
            impact_level = "medium",
            "MCP service performance degraded"
        );
    }
}

fn log_health_check_error(config: &McpServerConfig, error: &crate::error::McpDomainError) {
    tracing::info!(
        service_name = %config.name,
        error = %error,
        check_type = "continuous_monitoring",
        "Health check failed"
    );
    tracing::error!(service_name = %config.name, error = %error, "Health check failed for service");
}

fn get_error_message(error_message: Option<&String>) -> &str {
    error_message
        .map(String::as_str)
        .filter(|e| !e.is_empty())
        .unwrap_or("[no error message]")
}
