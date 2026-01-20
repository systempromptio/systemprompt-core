use crate::cli_settings::CliConfig;
use anyhow::Result;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_core_scheduler::{ProcessCleanup, ServiceManagementService};
use systemprompt_models::ProfileBootstrap;
use systemprompt_runtime::AppContext;

use super::start::ServiceTarget;

const DEFAULT_API_PORT: u16 = 8080;

fn get_api_port() -> u16 {
    ProfileBootstrap::get()
        .map(|p| p.server.port)
        .unwrap_or(DEFAULT_API_PORT)
}

pub async fn execute(target: ServiceTarget, force: bool, _config: &CliConfig) -> Result<()> {
    let ctx = Arc::new(AppContext::new().await?);
    let service_mgmt = ServiceManagementService::new(Arc::clone(ctx.db_pool()));

    if target.mcp {
        CliService::section("Stopping MCP Servers");
        stop_mcp_servers(&service_mgmt, force).await?;
    }

    if target.agents {
        CliService::section("Stopping Agents");
        stop_agents(&service_mgmt, force).await?;
    }

    if target.api {
        CliService::section("Stopping API Server");
        stop_api(force).await?;
    }

    CliService::success("All requested services stopped");
    Ok(())
}

async fn stop_api(force: bool) -> Result<()> {
    let port = get_api_port();

    if let Some(pid) = ProcessCleanup::check_port(port) {
        CliService::info(&format!("Stopping API server (PID: {})...", pid));
        if force {
            ProcessCleanup::kill_process(pid);
        } else {
            ProcessCleanup::terminate_gracefully(pid, 100).await;
        }
    }

    ProcessCleanup::kill_port(port);
    ProcessCleanup::wait_for_port_free(port, 5, 200).await?;

    CliService::success("API server stopped");
    Ok(())
}

async fn stop_agents(service_mgmt: &ServiceManagementService, force: bool) -> Result<()> {
    let agents = service_mgmt.get_services_by_type("agent").await?;

    if agents.is_empty() {
        CliService::info("No agents running");
        return Ok(());
    }

    let mut stopped = 0;
    for agent in &agents {
        CliService::info(&format!("Stopping {}...", agent.name));
        service_mgmt.stop_service(agent, force).await?;
        stopped += 1;
    }

    CliService::success(&format!("Stopped {} agents", stopped));
    Ok(())
}

async fn stop_mcp_servers(service_mgmt: &ServiceManagementService, force: bool) -> Result<()> {
    let servers = service_mgmt.get_services_by_type("mcp").await?;

    if servers.is_empty() {
        CliService::info("No MCP servers running");
        return Ok(());
    }

    let mut stopped = 0;
    for server in &servers {
        CliService::info(&format!("Stopping {}...", server.name));
        service_mgmt.stop_service(server, force).await?;
        stopped += 1;
    }

    CliService::success(&format!("Stopped {} MCP servers", stopped));
    Ok(())
}
