//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::cli_settings::CliConfig;
use crate::shared::CommandOutput;
use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_logging::CliService;
use systemprompt_mcp::services::McpOrchestrator;
use systemprompt_mcp::{HealthStatus, McpServiceStatus};
use systemprompt_models::mcp::McpServerType;
use systemprompt_runtime::{AppContext, StartupValidator, display_validation_report};
use systemprompt_scheduler::{
    RuntimeStatus, ServiceStateVerifier, ServiceType, VerifiedServiceState,
};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(super) struct ServiceStatusOutput {
    pub services: Vec<ServiceStatusRow>,
    pub summary: StatusSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(super) struct ServiceStatusRow {
    pub name: String,
    pub service_type: String,
    pub status: String,
    pub pid: Option<u32>,
    pub port: u16,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(super) struct StatusSummary {
    pub total: usize,
    pub running: usize,
    pub stopped: usize,
}

impl From<&VerifiedServiceState> for ServiceStatusRow {
    fn from(state: &VerifiedServiceState) -> Self {
        Self {
            name: state.name.clone(),
            service_type: state.service_type.to_string(),
            status: state.status_display().to_owned(),
            pid: state.pid,
            port: state.port,
            action: state.action_display().to_owned(),
            error: state.error.clone(),
            health: None,
            endpoint: None,
        }
    }
}

fn health_label(health: HealthStatus) -> String {
    if matches!(health, HealthStatus::Healthy | HealthStatus::Degraded) {
        "OK".to_owned()
    } else {
        "DEGRADED".to_owned()
    }
}

fn external_row(status: &McpServiceStatus) -> ServiceStatusRow {
    ServiceStatusRow {
        name: status.name.clone(),
        service_type: "mcp".to_owned(),
        status: "remote".to_owned(),
        pid: None,
        port: 0,
        action: "none".to_owned(),
        error: None,
        health: Some(health_label(status.health)),
        endpoint: status.endpoint.clone(),
    }
}

fn build_status_output(
    states: &[VerifiedServiceState],
    mcp_health: &HashMap<String, HealthStatus>,
    external: &[ServiceStatusRow],
    include_health: bool,
) -> ServiceStatusOutput {
    let mut services: Vec<ServiceStatusRow> = states
        .iter()
        .map(|state| {
            let mut row = ServiceStatusRow::from(state);
            if include_health {
                row.health = Some(managed_health_label(state, mcp_health));
            }
            row
        })
        .collect();

    services.extend(external.iter().cloned());

    let managed_running = states
        .iter()
        .filter(|s| s.runtime_status == RuntimeStatus::Running)
        .count();
    let external_running = external
        .iter()
        .filter(|r| r.health.as_deref() == Some("OK"))
        .count();
    let running = managed_running + external_running;
    let total = states.len() + external.len();

    ServiceStatusOutput {
        services,
        summary: StatusSummary {
            total,
            running,
            stopped: total - running,
        },
    }
}

fn managed_health_label(
    state: &VerifiedServiceState,
    mcp_health: &HashMap<String, HealthStatus>,
) -> String {
    if state.service_type == ServiceType::Mcp {
        return mcp_health
            .get(&state.name)
            .map_or_else(|| "DEGRADED".to_owned(), |h| health_label(*h));
    }
    if state.is_healthy() {
        "OK".to_owned()
    } else {
        "DEGRADED".to_owned()
    }
}

pub(super) async fn execute(
    detailed: bool,
    json: bool,
    health: bool,
    config: &CliConfig,
) -> Result<CommandOutput> {
    let ctx = Arc::new(AppContext::new().await?);

    let Ok(configs) = super::load_service_configs() else {
        let mut validator = StartupValidator::new();
        let report = validator.validate(ctx.config());
        if report.has_errors() {
            display_validation_report(&report);
            return Err(anyhow::anyhow!("Startup validation failed"));
        }
        return Err(anyhow::anyhow!("Failed to load service configs"));
    };

    let state_manager = ServiceStateVerifier::new(Arc::clone(ctx.db_pool()));
    let states = state_manager.get_verified_states(&configs).await?;

    let mcp_statuses = mcp_service_statuses(&ctx).await?;
    let mcp_health: HashMap<String, HealthStatus> = mcp_statuses
        .iter()
        .filter(|s| s.server_type == McpServerType::Internal)
        .map(|s| (s.name.clone(), s.health))
        .collect();
    let external: Vec<ServiceStatusRow> = mcp_statuses
        .iter()
        .filter(|s| s.server_type == McpServerType::External)
        .map(external_row)
        .collect();

    let output = build_status_output(&states, &mcp_health, &external, health);

    let result = CommandOutput::table_of(
        vec![
            "name",
            "service_type",
            "status",
            "pid",
            "endpoint",
            "action",
        ],
        &output.services,
    )
    .with_title("Service Status");

    if json || config.is_json_output() {
        return Ok(result);
    }

    if detailed {
        output_detailed(&states, &mcp_health, &external, health);
    } else {
        render_table_output(&output);
    }

    Ok(result.with_skip_render())
}

async fn mcp_service_statuses(ctx: &Arc<AppContext>) -> Result<Vec<McpServiceStatus>> {
    let manager = McpOrchestrator::new(
        Arc::clone(ctx.db_pool()),
        Arc::clone(ctx.app_paths_arc()),
        ctx.mcp_registry().clone(),
    )?;
    Ok(manager.service_statuses().await?)
}

fn render_table_output(output: &ServiceStatusOutput) {
    CliService::section("Service Status");

    for service in &output.services {
        let pid_str = service
            .pid
            .map_or_else(|| "-".to_owned(), |p| p.to_string());
        let locator = service
            .endpoint
            .as_deref()
            .map_or_else(|| format!("PID: {pid_str}"), |e| format!("endpoint: {e}"));
        CliService::key_value(
            &service.name,
            &format!(
                "{} | {} | {} | {}",
                service.service_type, service.status, locator, service.action
            ),
        );
    }

    CliService::info(&format!(
        "{}/{} services running",
        output.summary.running, output.summary.total
    ));
}

fn output_detailed(
    states: &[VerifiedServiceState],
    mcp_health: &HashMap<String, HealthStatus>,
    external: &[ServiceStatusRow],
    include_health: bool,
) {
    for state in states {
        CliService::section(&state.name);
        CliService::key_value("Type", &state.service_type.to_string());
        CliService::key_value("Status", state.status_display());
        CliService::key_value("Port", &state.port.to_string());

        if let Some(pid) = state.pid {
            CliService::key_value("PID", &pid.to_string());
        }

        CliService::key_value("Desired", &format!("{:?}", state.desired_status));
        CliService::key_value("Action", state.action_display());

        if let Some(error) = &state.error {
            CliService::error(&format!("Error: {}", error));
        }

        if include_health {
            CliService::key_value("Health", &managed_health_label(state, mcp_health));
        }
    }

    for row in external {
        CliService::section(&row.name);
        CliService::key_value("Type", &row.service_type);
        CliService::key_value("Status", &row.status);
        if let Some(endpoint) = &row.endpoint {
            CliService::key_value("Endpoint", endpoint);
        }
        if let Some(health) = &row.health {
            CliService::key_value("Health", health);
        }
    }
}
