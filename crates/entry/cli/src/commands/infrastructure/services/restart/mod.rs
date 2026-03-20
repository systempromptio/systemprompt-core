mod batch;
mod single;

use anyhow::{Context, Result};
use std::sync::Arc;
use systemprompt_agent::AgentState;
use systemprompt_agent::services::agent_orchestration::AgentOrchestrator;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_logging::CliService;
use systemprompt_models::ProfileBootstrap;
use systemprompt_oauth::JwtValidationProviderImpl;
use systemprompt_runtime::AppContext;

pub use batch::{execute_all_agents, execute_all_mcp, execute_failed};
pub use single::{execute_agent, execute_api, execute_mcp};

const DEFAULT_API_PORT: u16 = 8080;

pub(crate) fn create_agent_state(ctx: &AppContext) -> Result<Arc<AgentState>> {
    let jwt_provider = Arc::new(
        JwtValidationProviderImpl::from_config().context("Failed to create JWT provider")?,
    );
    Ok(Arc::new(AgentState::new(
        Arc::clone(ctx.db_pool()),
        Arc::new(ctx.config().clone()),
        jwt_provider,
    )))
}

pub(crate) fn get_api_port() -> u16 {
    ProfileBootstrap::get().map_or(DEFAULT_API_PORT, |p| p.server.port)
}

pub(crate) async fn resolve_name(agent_identifier: &str) -> Result<String> {
    let registry = AgentRegistry::new().await?;
    let agent = registry.get_agent(agent_identifier).await?;
    Ok(agent.name)
}

pub(crate) async fn create_orchestrator(ctx: &Arc<AppContext>) -> Result<AgentOrchestrator> {
    let agent_state = create_agent_state(ctx)?;
    AgentOrchestrator::new(agent_state, None)
        .await
        .context("Failed to initialize agent orchestrator")
}

pub(crate) fn format_batch_message(
    service_label: &str,
    restarted: usize,
    failed: usize,
    quiet: bool,
) -> String {
    match (restarted, failed) {
        (0, 0) => {
            let msg = format!("No enabled {} found", service_label);
            if !quiet {
                CliService::info(&msg);
            }
            msg
        },
        (r, 0) => {
            let msg = format!("Restarted {} {}", r, service_label);
            if !quiet {
                CliService::success(&msg);
            }
            msg
        },
        (0, f) => {
            let msg = format!("Failed to restart {} {}", f, service_label);
            if !quiet {
                CliService::warning(&msg);
            }
            msg
        },
        (r, f) => {
            if !quiet {
                CliService::success(&format!("Restarted {} {}", r, service_label));
                CliService::warning(&format!("Failed to restart {} {}", f, service_label));
            }
            format!("Restarted {} {}, {} failed", r, service_label, f)
        },
    }
}
