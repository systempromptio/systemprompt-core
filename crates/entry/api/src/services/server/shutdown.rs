//! Graceful shutdown: signal wait, child termination, forced-exit backstop.
//!
//! Ordering matters. Axum starts draining connections only once
//! [`shutdown_signal`] resolves, so the run loop bounds that drain with
//! [`join_within_drain_grace`] and arms the hard [`arm_forced_exit`] deadline
//! only afterwards — a single deadline spanning both would let a wedged SSE
//! stream consume the whole budget and kill the process before any child was
//! signalled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::{ProcessCleanup, SchedulerHandle};

const CHILD_SHUTDOWN_GRACE_MS: u64 = 5_000;
const AXUM_DRAIN_GRACE_MS: u64 = 10_000;
const FORCED_SHUTDOWN_GRACE_MS: u64 = 10_000;

#[cfg(feature = "test-api")]
pub mod test_api {
    use systemprompt_runtime::AppContext;

    pub async fn drain(ctx: &AppContext) {
        super::drain(ctx, None).await;
    }

    pub async fn terminate_children(ctx: &AppContext) {
        super::terminate_children(ctx).await;
    }

    pub async fn join_within_drain_grace(
        serve: impl Future<Output = anyhow::Result<()>>,
    ) -> anyhow::Result<()> {
        super::join_within_drain_grace(serve).await
    }

    pub const AXUM_DRAIN_GRACE_MS: u64 = super::AXUM_DRAIN_GRACE_MS;
    pub const CHILD_SHUTDOWN_GRACE_MS: u64 = super::CHILD_SHUTDOWN_GRACE_MS;
}

pub(super) async fn shutdown_signal() {
    wait_for_signal().await;
    super::readiness::signal_shutdown();
    arm_exit_on_second_signal();
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

/// Exits on a second shutdown signal, however far along the drain is. Armed as
/// soon as the first signal lands, so a second Ctrl-C is always honoured.
///
/// Deliberately has no timer: while axum is still draining, a deadline that
/// fired here would kill the process before [`drain`] ever ran and strand every
/// MCP and agent child. That window is bounded by
/// [`join_within_drain_grace`] instead, which abandons the drain rather than
/// the process.
fn arm_exit_on_second_signal() {
    tokio::spawn(async {
        wait_for_signal().await;
        tracing::warn!("Second shutdown signal received, forcing immediate exit");
        force_exit();
    });
}

/// Backstop for the post-drain teardown in [`drain`]. Armed by the run loop
/// only once axum's drain has finished, so the whole grace window is available
/// to child termination.
pub(super) fn arm_forced_exit() {
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(FORCED_SHUTDOWN_GRACE_MS)).await;
        tracing::warn!(
            grace_ms = FORCED_SHUTDOWN_GRACE_MS,
            "Shutdown teardown exceeded grace window, forcing exit"
        );
        force_exit();
    });
}

#[expect(
    clippy::exit,
    reason = "forced process exit is the explicit purpose of the shutdown backstops"
)]
fn force_exit() -> ! {
    std::process::exit(0);
}

/// Waits for axum's graceful drain, giving up after [`AXUM_DRAIN_GRACE_MS`].
///
/// SSE streams never close on their own, so an unbounded wait starves the child
/// termination that follows. Abandoning the drain is the lesser evil: the
/// listener is already closed and readiness already reports shutting-down, so
/// the alternative is leaving MCP and agent subprocesses running on their ports
/// for the next boot to reclaim.
pub(super) async fn join_within_drain_grace(
    serve: impl Future<Output = anyhow::Result<()>>,
) -> anyhow::Result<()> {
    tokio::select! {
        result = serve => result,
        () = tokio::time::sleep(Duration::from_millis(AXUM_DRAIN_GRACE_MS)) => {
            tracing::warn!(
                grace_ms = AXUM_DRAIN_GRACE_MS,
                "Connection drain exceeded grace window, proceeding to terminate children"
            );
            Ok(())
        },
    }
}

pub(super) async fn drain(ctx: &AppContext, scheduler: Option<SchedulerHandle>) {
    if let Some(handle) = ctx.event_bridge().get() {
        handle.abort();
    }

    if let Some(handle) = scheduler
        && let Err(e) = handle.shutdown().await
    {
        tracing::warn!(error = %e, "Scheduler failed to drain cleanly");
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

    tokio::join!(
        terminate_agent_children(&repo),
        terminate_mcp_children(&repo),
    );
}

async fn terminate_agent_children(repo: &systemprompt_database::ServiceRepository) {
    use systemprompt_models::subprocess::AGENT_NAME_ENV;

    let names = match repo.list_all_agent_service_names().await {
        Ok(names) => names,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list agent services for shutdown");
            return;
        },
    };

    futures_util::future::join_all(names.into_iter().map(|name| async move {
        if let Ok(Some(service)) = repo.find_service_by_name(&name).await {
            terminate_service_child(repo, &name, service.pid, AGENT_NAME_ENV).await;
        }
    }))
    .await;
}

async fn terminate_mcp_children(repo: &systemprompt_database::ServiceRepository) {
    use systemprompt_models::subprocess::MCP_SERVICE_ID_ENV;

    let services = match repo.list_mcp_services().await {
        Ok(services) => services,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list MCP services for shutdown");
            return;
        },
    };

    futures_util::future::join_all(services.into_iter().map(|service| async move {
        terminate_service_child(repo, &service.name, service.pid, MCP_SERVICE_ID_ENV).await;
    }))
    .await;
}

// Why: Group-kills a recorded child only after confirming the live PID is still
// that child. A recycled PID is cleared without signalling — `kill(-pid)` on
// it would hit every process in the reused group, e.g. the systemd
// `user@<uid>` session leader.
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

    if !systemprompt_models::subprocess::live_pid_is_subprocess(pid, name_key, name) {
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
