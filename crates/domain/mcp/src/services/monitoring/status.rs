use crate::services::monitoring::health::{perform_health_check, HealthStatus};
use crate::{McpServerConfig, ERROR, RUNNING, STOPPED};
use anyhow::Result;
use std::collections::HashMap;
use std::hash::BuildHasher;

#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub state: String,
    pub pid: Option<u32>,
    pub health: String,
    pub uptime_seconds: Option<i64>,
    pub tools_count: usize,
    pub latency_ms: Option<u32>,
    pub auth_required: bool,
}

pub async fn get_all_service_status(
    servers: &[McpServerConfig],
) -> Result<HashMap<String, ServiceStatus>> {
    let mut status_map = HashMap::new();

    for server in servers {
        let status = get_service_status(server).await?;
        status_map.insert(server.name.clone(), status);
    }

    Ok(status_map)
}

async fn get_service_status(config: &McpServerConfig) -> Result<ServiceStatus> {
    match perform_health_check(config).await {
        Ok(health_result) => {
            let state = match health_result.status {
                HealthStatus::Healthy | HealthStatus::Degraded => RUNNING.to_string(),
                HealthStatus::Unhealthy => STOPPED.to_string(),
                HealthStatus::Unknown => ERROR.to_string(),
            };

            Ok(ServiceStatus {
                state,
                pid: None,
                health: health_result.status.as_str().to_string(),
                uptime_seconds: None,
                tools_count: health_result.details.tools_available,
                latency_ms: Some(health_result.latency_ms),
                auth_required: config.oauth.required,
            })
        },
        Err(_) => Ok(ServiceStatus {
            state: STOPPED.to_string(),
            pid: None,
            health: "unreachable".to_string(),
            uptime_seconds: None,
            tools_count: 0,
            latency_ms: None,
            auth_required: config.oauth.required,
        }),
    }
}

pub fn display_service_status<S: BuildHasher>(
    servers: &[McpServerConfig],
    status_data: &HashMap<String, ServiceStatus, S>,
) {
    if servers.is_empty() {
        tracing::info!("No MCP services configured");
        return;
    }

    let mut running_count = 0;
    let mut error_count = 0;

    for server in servers {
        if let Some(s) = status_data.get(&server.name) {
            match s.state.as_str() {
                RUNNING => running_count += 1,
                ERROR => error_count += 1,
                _ => {},
            }
        }
    }

    tracing::info!(
        running = running_count,
        error = error_count,
        total = servers.len(),
        "MCP services status"
    );
}
