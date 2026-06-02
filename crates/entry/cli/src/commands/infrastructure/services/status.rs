use crate::cli_settings::CliConfig;
use crate::shared::CommandOutput;
use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, StartupValidator, display_validation_report};
use systemprompt_scheduler::{RuntimeStatus, ServiceStateVerifier, VerifiedServiceState};

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
        }
    }
}

fn build_status_output(
    states: &[VerifiedServiceState],
    include_health: bool,
) -> ServiceStatusOutput {
    let running = states
        .iter()
        .filter(|s| s.runtime_status == RuntimeStatus::Running)
        .count();
    let total = states.len();

    let services: Vec<ServiceStatusRow> = states
        .iter()
        .map(|state| {
            let mut row = ServiceStatusRow::from(state);
            if include_health {
                row.health = Some(if state.is_healthy() {
                    "OK".to_owned()
                } else {
                    "DEGRADED".to_owned()
                });
            }
            row
        })
        .collect();

    ServiceStatusOutput {
        services,
        summary: StatusSummary {
            total,
            running,
            stopped: total - running,
        },
    }
}

fn execute_command(states: &[VerifiedServiceState], include_health: bool) -> CommandOutput {
    let output = build_status_output(states, include_health);

    CommandOutput::table_of(
        vec!["name", "service_type", "status", "pid", "action"],
        &output.services,
    )
    .with_title("Service Status")
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

    let result = execute_command(&states, health);

    if json || config.is_json_output() {
        return Ok(result);
    }

    if detailed {
        output_detailed(&states, health);
    } else {
        render_table_output(&states, health);
    }

    Ok(result.with_skip_render())
}

fn render_table_output(states: &[VerifiedServiceState], include_health: bool) {
    let output = build_status_output(states, include_health);

    CliService::section("Service Status");

    for service in &output.services {
        let pid_str = service
            .pid
            .map_or_else(|| "-".to_owned(), |p| p.to_string());
        CliService::key_value(
            &service.name,
            &format!(
                "{} | {} | PID: {} | {}",
                service.service_type, service.status, pid_str, service.action
            ),
        );
    }

    CliService::info(&format!(
        "{}/{} services running",
        output.summary.running, output.summary.total
    ));
}

fn output_detailed(states: &[VerifiedServiceState], include_health: bool) {
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
            let health_status = if state.is_healthy() { "OK" } else { "DEGRADED" };
            CliService::key_value("Health", health_status);
        }
    }
}
