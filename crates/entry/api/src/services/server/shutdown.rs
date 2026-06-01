use std::time::Duration;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{ProcessCleanup, SchedulerHandle};

const CHILD_SHUTDOWN_GRACE_MS: u64 = 5_000;
const FORCED_SHUTDOWN_GRACE_MS: u64 = 10_000;

pub(super) async fn shutdown_signal() {
    wait_for_signal().await;
    super::readiness::signal_shutdown();
    arm_forced_exit();
}

async fn wait_for_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "Failed to install Ctrl-C handler");
            std::future::pending::<()>().await;
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
}

/// Forces exit if axum's graceful drain wedges on a long-lived connection
/// (SSE streams never close on their own); the clean path abandons this
/// detached task instead.
#[expect(
    clippy::exit,
    reason = "forced process exit is the explicit purpose of this guard when graceful drain \
              wedges on a long-lived connection"
)]
fn arm_forced_exit() {
    tokio::spawn(async {
        tokio::select! {
            () = wait_for_signal() => {
                tracing::warn!("Second shutdown signal received, forcing immediate exit");
            },
            () = tokio::time::sleep(Duration::from_millis(FORCED_SHUTDOWN_GRACE_MS)) => {
                tracing::warn!(
                    grace_ms = FORCED_SHUTDOWN_GRACE_MS,
                    "Graceful shutdown exceeded grace window, forcing exit"
                );
            },
        }
        std::process::exit(0);
    });
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
    use systemprompt_models::subprocess::AGENT_NAME_ENV;

    let names = match repo.get_all_agent_service_names().await {
        Ok(names) => names,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list agent services for shutdown");
            return;
        },
    };

    for name in names {
        if let Ok(Some(service)) = repo.get_service_by_name(&name).await {
            terminate_service_child(repo, &name, service.pid, AGENT_NAME_ENV).await;
        }
    }
}

async fn terminate_mcp_children(repo: &systemprompt_database::ServiceRepository) {
    use systemprompt_models::subprocess::MCP_SERVICE_ID_ENV;

    let services = match repo.get_mcp_services().await {
        Ok(services) => services,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list MCP services for shutdown");
            return;
        },
    };

    for service in services {
        terminate_service_child(repo, &service.name, service.pid, MCP_SERVICE_ID_ENV).await;
    }
}

/// Group-kills a recorded child only after confirming the live PID is still
/// that child. A recycled PID is cleared without signalling — `kill(-pid)` on
/// it would hit every process in the reused group, e.g. the systemd
/// `user@<uid>` session leader.
async fn terminate_service_child(
    repo: &systemprompt_database::ServiceRepository,
    name: &str,
    pid: Option<i32>,
    name_key: &str,
) {
    let Some(pid) = pid.and_then(|p| u32::try_from(p).ok()) else {
        return;
    };
    if !ProcessCleanup::process_exists(pid) {
        return;
    }

    if !pid_is_our_child(pid, name_key, name) {
        tracing::warn!(
            service = %name,
            pid,
            "Recorded PID is alive but is not our child (recycled/stale); clearing registry row without signalling"
        );
        if let Err(e) = repo.update_service_stopped(name).await {
            tracing::warn!(service = %name, error = %e, "Failed to clear stale service PID");
        }
        return;
    }

    if ProcessCleanup::terminate_group_gracefully(pid, CHILD_SHUTDOWN_GRACE_MS).await {
        tracing::info!(service = %name, pid, "Terminated child process group on shutdown");
    } else {
        tracing::warn!(service = %name, pid, "Child process group survived shutdown signal");
    }
}

/// Fail-closed: a PID whose `/proc/<pid>/environ` markers cannot be read and
/// matched is reported as not-ours, so an unverified process is never
/// signalled.
#[cfg(target_os = "linux")]
fn pid_is_our_child(pid: u32, name_key: &str, name: &str) -> bool {
    match std::fs::read(format!("/proc/{pid}/environ")) {
        Ok(environ) => {
            systemprompt_models::subprocess::environ_identifies_child(&environ, name_key, name)
        },
        Err(e) => {
            tracing::warn!(pid, error = %e, "Could not read process environ to verify child identity");
            false
        },
    }
}

#[cfg(not(target_os = "linux"))]
fn pid_is_our_child(_pid: u32, _name_key: &str, _name: &str) -> bool {
    // Identity verification reads `/proc/<pid>/environ`, which only exists on
    // Linux — the sole server target. Off Linux, refuse to claim ownership so
    // the recycled-PID group-kill can never fire.
    false
}
