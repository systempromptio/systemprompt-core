use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{ProcessCleanup, SchedulerHandle};

const CHILD_SHUTDOWN_GRACE_MS: u64 = 5_000;

pub(super) async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "Failed to install Ctrl-C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            },
            Err(e) => {
                tracing::error!(error = %e, "Failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            },
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => tracing::info!("Received Ctrl-C, shutting down"),
        () = terminate => tracing::info!("Received SIGTERM, shutting down"),
    }

    super::readiness::signal_shutdown();
}

pub(super) async fn drain(ctx: &AppContext, scheduler: Option<SchedulerHandle>) {
    if let Some(handle) = scheduler {
        if let Err(e) = handle.shutdown().await {
            tracing::warn!(error = %e, "Scheduler failed to drain cleanly");
        }
    }

    terminate_children(ctx).await;
}

async fn terminate_children(ctx: &AppContext) {
    use systemprompt_database::ServiceRepository;

    let repo = match ServiceRepository::new(ctx.db_pool()) {
        Ok(repo) => repo,
        Err(e) => {
            tracing::warn!(error = %e, "Cannot reach service registry to terminate children");
            return;
        },
    };

    terminate_agent_children(&repo).await;
    terminate_mcp_children(&repo).await;
}

async fn terminate_agent_children(repo: &systemprompt_database::ServiceRepository) {
    let names = match repo.get_all_agent_service_names().await {
        Ok(names) => names,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list agent services for shutdown");
            return;
        },
    };

    for name in names {
        if let Ok(Some(service)) = repo.get_service_by_name(&name).await {
            terminate_pid_group(&name, service.pid).await;
        }
    }
}

async fn terminate_mcp_children(repo: &systemprompt_database::ServiceRepository) {
    let services = match repo.get_mcp_services().await {
        Ok(services) => services,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list MCP services for shutdown");
            return;
        },
    };

    for service in services {
        terminate_pid_group(&service.name, service.pid).await;
    }
}

async fn terminate_pid_group(name: &str, pid: Option<i32>) {
    let Some(pid) = pid.and_then(|p| u32::try_from(p).ok()) else {
        return;
    };
    if !ProcessCleanup::process_exists(pid) {
        return;
    }
    if ProcessCleanup::terminate_group_gracefully(pid, CHILD_SHUTDOWN_GRACE_MS).await {
        tracing::info!(service = %name, pid, "Terminated child process group on shutdown");
    } else {
        tracing::warn!(service = %name, pid, "Child process group survived shutdown signal");
    }
}
