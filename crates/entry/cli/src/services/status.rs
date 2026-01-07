use anyhow::Result;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_core_scheduler::{RuntimeStatus, ServiceStateManager, VerifiedServiceState};
use systemprompt_runtime::AppContext;

pub async fn execute(detailed: bool, json: bool, health: bool) -> Result<()> {
    let ctx = Arc::new(AppContext::new().await?);
    let configs = super::load_service_configs(&ctx)?;
    let state_manager = ServiceStateManager::new(Arc::clone(ctx.db_pool()));

    let states = state_manager.get_verified_states(&configs).await?;

    if json {
        output_json(&states);
    } else if detailed {
        output_detailed(&states, health);
    } else {
        output_table(&states);
    }

    Ok(())
}

fn output_table(states: &[VerifiedServiceState]) {
    let headers = &["SERVICE", "TYPE", "STATUS", "PID", "ACTION"];
    let rows: Vec<Vec<String>> = states
        .iter()
        .map(|state| {
            let pid_str = state.pid.map_or_else(|| "-".to_string(), |p| p.to_string());

            vec![
                state.name.clone(),
                state.service_type.to_string(),
                state.status_display().to_string(),
                pid_str,
                state.action_display().to_string(),
            ]
        })
        .collect();

    CliService::table(headers, &rows);

    let running = states
        .iter()
        .filter(|s| s.runtime_status == RuntimeStatus::Running)
        .count();
    let total = states.len();

    CliService::info(&format!("{}/{} services running", running, total));
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

fn output_json(states: &[VerifiedServiceState]) {
    let states_vec: Vec<_> = states.to_vec();
    CliService::json(&states_vec);
}
