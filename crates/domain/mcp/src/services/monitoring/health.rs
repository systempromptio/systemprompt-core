use crate::models::ValidationResultType;
use crate::services::client::McpConnectionResult;
use crate::McpServerConfig;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::time::Duration;
use systemprompt_database::DbPool;
use tokio::time::{interval, timeout};
use tracing::Instrument;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

impl HealthStatus {
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unhealthy => "unhealthy",
            Self::Unknown => "unknown",
        }
    }

    pub const fn emoji(&self) -> &str {
        match self {
            Self::Healthy => "✅",
            Self::Degraded => "⚠️",
            Self::Unhealthy => "❌",
            Self::Unknown => "❓",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    pub status: HealthStatus,
    pub connection_result: Option<McpConnectionResult>,
    pub latency_ms: u32,
    pub details: HealthCheckDetails,
}

#[derive(Debug, Clone)]
pub struct HealthCheckDetails {
    pub service_name: String,
    pub tools_available: usize,
    pub requires_auth: bool,
    pub validation_type: String,
    pub error_message: Option<String>,
    pub server_version: Option<String>,
}

impl HealthCheckResult {
    pub fn from_connection_result(result: McpConnectionResult, config: &McpServerConfig) -> Self {
        let validation_type = ValidationResultType::parse(&result.validation_type);
        let status = if result.success {
            if result.connection_time_ms < 1000 {
                HealthStatus::Healthy
            } else {
                HealthStatus::Degraded
            }
        } else {
            match validation_type {
                ValidationResultType::AuthRequired => HealthStatus::Healthy,
                ValidationResultType::PortUnavailable
                | ValidationResultType::ConnectionFailed
                | ValidationResultType::Timeout => HealthStatus::Unhealthy,
                _ => HealthStatus::Unknown,
            }
        };

        let details = HealthCheckDetails {
            service_name: config.name.clone(),
            tools_available: result.tools_count,
            requires_auth: config.oauth.required,
            validation_type: validation_type.to_string(),
            error_message: result.error_message.clone(),
            server_version: result.server_info.as_ref().map(|info| info.version.clone()),
        };

        Self {
            status,
            latency_ms: result.connection_time_ms,
            connection_result: Some(result),
            details,
        }
    }

    pub fn unhealthy(config: &McpServerConfig, error: String) -> Self {
        Self {
            status: HealthStatus::Unhealthy,
            connection_result: None,
            latency_ms: 0,
            details: HealthCheckDetails {
                service_name: config.name.clone(),
                tools_available: 0,
                requires_auth: config.oauth.required,
                validation_type: ValidationResultType::Error.to_string(),
                error_message: Some(error),
                server_version: None,
            },
        }
    }
}

pub async fn check_service_health(config: &McpServerConfig) -> Result<HealthStatus> {
    let result = perform_health_check(config).await?;
    Ok(result.status)
}

pub async fn perform_health_check(config: &McpServerConfig) -> Result<HealthCheckResult> {
    use crate::services::client::validate_connection_with_auth;

    let connection_result = timeout(
        Duration::from_secs(30),
        validate_connection_with_auth(
            &config.name,
            &config.host,
            config.port,
            config.oauth.required,
        ),
    )
    .await;

    match connection_result {
        Ok(Ok(mcp_result)) => Ok(HealthCheckResult::from_connection_result(
            mcp_result, config,
        )),
        Ok(Err(e)) => Ok(HealthCheckResult::unhealthy(
            config,
            format!("Connection error: {e}"),
        )),
        Err(_) => Ok(HealthCheckResult::unhealthy(
            config,
            "Health check timeout".to_string(),
        )),
    }
}

struct HealthMonitorState {
    previous_status: Option<HealthStatus>,
    failure_count: u32,
    last_failure_time: Option<DateTime<Utc>>,
}

impl HealthMonitorState {
    const fn new() -> Self {
        Self {
            previous_status: None,
            failure_count: 0,
            last_failure_time: None,
        }
    }
}

pub async fn monitor_health_continuously(
    config: &McpServerConfig,
    report_interval: Duration,
    _db_pool: DbPool,
) -> Result<()> {
    let span: tracing::Span = systemprompt_logging::SystemSpan::new("mcp_health_monitor").into();
    async move {
        let mut ticker = interval(report_interval);
        let mut state = HealthMonitorState::new();

        loop {
            ticker.tick().await;

            match perform_health_check(config).await {
                Ok(result) => {
                    handle_health_result(config, &result, &mut state);
                    state.previous_status = Some(result.status);
                },
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

fn log_health_check_error(config: &McpServerConfig, error: &anyhow::Error) {
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
