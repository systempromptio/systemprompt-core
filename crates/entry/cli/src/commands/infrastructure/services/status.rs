use crate::cli_settings::CliConfig;
use crate::shared::{CommandResult, RenderingHints};
use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_core_scheduler::{RuntimeStatus, ServiceStateManager, VerifiedServiceState};
use systemprompt_runtime::{display_validation_report, AppContext, StartupValidator};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServiceStatusOutput {
    pub services: Vec<ServiceStatusRow>,
    pub summary: StatusSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServiceStatusRow {
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
pub struct StatusSummary {
    pub total: usize,
    pub running: usize,
    pub stopped: usize,
}

impl From<&VerifiedServiceState> for ServiceStatusRow {
    fn from(state: &VerifiedServiceState) -> Self {
        Self {
            name: state.name.clone(),
            service_type: state.service_type.to_string(),
            status: state.status_display().to_string(),
            pid: state.pid,
            port: state.port,
            action: state.action_display().to_string(),
            error: state.error.clone(),
            health: None,
        }
    }
}

pub fn execute_command(
    states: &[VerifiedServiceState],
    include_health: bool,
) -> CommandResult<ServiceStatusOutput> {
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
                    "OK".to_string()
                } else {
                    "DEGRADED".to_string()
                });
            }
            row
        })
        .collect();

    let output = ServiceStatusOutput {
        services,
        summary: StatusSummary {
            total,
            running,
            stopped: total - running,
        },
    };

    CommandResult::table(output)
        .with_title("Service Status")
        .with_hints(RenderingHints {
            columns: Some(vec![
                "name".to_string(),
                "service_type".to_string(),
                "status".to_string(),
                "pid".to_string(),
                "action".to_string(),
            ]),
            ..Default::default()
        })
}

pub async fn execute(detailed: bool, json: bool, health: bool, config: &CliConfig) -> Result<()> {
    let ctx = Arc::new(AppContext::new().await?);

    let Ok(configs) = super::load_service_configs(&ctx) else {
        let mut validator = StartupValidator::new();
        let report = validator.validate(ctx.config());
        if report.has_errors() {
            display_validation_report(&report);
            return Err(anyhow::anyhow!("Startup validation failed"));
        }
        return Err(anyhow::anyhow!("Failed to load service configs"));
    };

    let state_manager = ServiceStateManager::new(Arc::clone(ctx.db_pool()));

    let states = state_manager.get_verified_states(&configs).await?;

    if json || config.is_json_output() {
        let result = execute_command(&states, health);
        CliService::json(&result);
    } else if detailed {
        output_detailed(&states, health);
    } else {
        render_table_output(&states, health);
    }

    Ok(())
}

fn render_table_output(states: &[VerifiedServiceState], include_health: bool) {
    let result = execute_command(states, include_health);

    if let Some(title) = &result.title {
        CliService::section(title);
    }

    for service in &result.data.services {
        let pid_str = service
            .pid
            .map_or_else(|| "-".to_string(), |p| p.to_string());
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
        result.data.summary.running, result.data.summary.total
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
