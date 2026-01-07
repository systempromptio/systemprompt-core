use anyhow::Result;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_core_scheduler::{ProcessCleanup, ServiceManagementService};
use systemprompt_runtime::AppContext;

use super::start::ServiceTarget;

pub async fn execute(target: ServiceTarget, force: bool) -> Result<()> {

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
    if let Some(pid) = ProcessCleanup::check_port(8080) {
        CliService::info(&format!("Stopping API server (PID: {})...", pid));
        if force {
            ProcessCleanup::kill_process(pid);
        } else {
            ProcessCleanup::terminate_gracefully(pid, 100).await;
        }
    }

    ProcessCleanup::kill_port(8080);
    ProcessCleanup::wait_for_port_free(8080, 5, 200).await?;

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
