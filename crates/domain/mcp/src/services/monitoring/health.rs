//! Health checking and continuous monitoring for MCP servers.
//!
//! Defines [`HealthStatus`] and [`HealthCheckResult`], maps a connection probe
//! into a health verdict (latency-aware, with OAuth-gated servers treated as
//! healthy when reachable), and runs a long-lived monitor that logs degradation
//! and recovery transitions on a fixed interval. Accessor-backed external
//! servers are reported healthy without probing: their bearer is minted
//! per-user on demand, so the monitor has no credential to authenticate with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::McpServerConfig;
use crate::error::McpDomainResult;
use crate::models::ValidationResultType;
use crate::services::client::McpConnectionResult;
use std::time::Duration;
use tokio::time::timeout;

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

    pub fn external_accessor_backed(config: &McpServerConfig) -> Self {
        Self {
            status: HealthStatus::Healthy,
            connection_result: None,
            latency_ms: 0,
            details: HealthCheckDetails {
                service_name: config.name.clone(),
                tools_available: 0,
                requires_auth: config.oauth.required,
                validation_type: "external_accessor_backed".to_owned(),
                error_message: None,
                server_version: None,
            },
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

pub async fn check_service_health(config: &McpServerConfig) -> McpDomainResult<HealthStatus> {
    let result = perform_health_check(config).await?;
    Ok(result.status)
}

pub async fn perform_health_check(config: &McpServerConfig) -> McpDomainResult<HealthCheckResult> {
    use crate::services::client::{validate_connection_by_url, validate_connection_with_auth};
    use systemprompt_models::mcp::McpServerType;

    if matches!(config.server_type, McpServerType::External) && config.external_auth.is_some() {
        return Ok(HealthCheckResult::external_accessor_backed(config));
    }

    let connection_result = match config.server_type {
        McpServerType::Internal => {
            timeout(
                Duration::from_secs(30),
                validate_connection_with_auth(
                    &config.name,
                    &config.host,
                    config.port,
                    config.oauth.required,
                ),
            )
            .await
        },
        McpServerType::External => {
            timeout(
                Duration::from_secs(30),
                validate_connection_by_url(&config.name, &config.remote_endpoint),
            )
            .await
        },
    };

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
            "Health check timeout".to_owned(),
        )),
    }
}
