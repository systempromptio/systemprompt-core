//! Graceful shutdown: signal wait, child termination, forced-exit backstop.
//!
//! The wait resolves on `SIGTERM`, `Ctrl-C`, or the [`ShutdownRequest`] the
//! listener was bound with — the handle an admin action raises when it needs
//! the supervisor to restart the process on new content.
//!
//! Ordering matters. Axum starts draining connections only once
//! `shutdown_signal` resolves, so the run loop bounds that drain with
//! [`join_within_drain_grace`] and arms the hard `arm_forced_exit` deadline
//! only afterwards — a single deadline spanning both would let a wedged SSE
//! stream consume the whole budget and kill the process before any child was
//! signalled. [`drain`] then shuts the process's
//! [`BackgroundTasks`](systemprompt_traits::BackgroundTasks) down alongside
//! child termination and before the log writer flushes, so post-response
//! audit and analytics writes still land. A forced exit reports a non-zero
//! status: the teardown did not complete.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;
use systemprompt_loader::subprocess::{ChildKind, StopOutcome};
use systemprompt_runtime::{AppContext, ShutdownRequest};
use systemprompt_scheduler::SchedulerHandle;
use systemprompt_traits::DrainOutcome;

pub const CHILD_SHUTDOWN_GRACE_MS: u64 = 5_000;
pub const AXUM_DRAIN_GRACE_MS: u64 = 10_000;
pub const BACKGROUND_DRAIN_GRACE_MS: u64 = 5_000;
const FORCED_SHUTDOWN_GRACE_MS: u64 = 10_000;
const FORCED_EXIT_CODE: i32 = 1;

pub(super) async fn shutdown_signal(restart: ShutdownRequest) {
    wait_for_signal(&restart).await;
    super::readiness::signal_shutdown();
    arm_exit_on_second_signal(restart);
}

async fn wait_for_signal(restart: &ShutdownRequest) {
    let restart_requested = restart.requested();

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
        () = restart_requested => tracing::info!("Restart requested in-process, shutting down"),
    }
}

fn arm_exit_on_second_signal(restart: ShutdownRequest) {
    tokio::spawn(async move {
        wait_for_signal(&restart).await;
        tracing::warn!("Second shutdown signal received, forcing immediate exit");
        force_exit();
    });
}

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
    std::process::exit(FORCED_EXIT_CODE);
}

pub async fn join_within_drain_grace(
    serve: impl Future<Output = anyhow::Result<()>>,
) -> anyhow::Result<()> {
    use super::readiness::ReadinessEvent;
    use tokio::sync::broadcast::error::RecvError;

    let mut serve = std::pin::pin!(serve);
    let mut readiness = super::readiness::get_readiness_receiver();

    loop {
        tokio::select! {
            result = &mut serve => return result,
            event = readiness.recv() => match event {
                Ok(ReadinessEvent::ApiShuttingDown) => break,
                Ok(ReadinessEvent::ApiReady) | Err(RecvError::Lagged(_)) => (),
                Err(RecvError::Closed) => return serve.await,
            },
        }
    }

    tokio::select! {
        result = &mut serve => result,
        () = tokio::time::sleep(Duration::from_millis(AXUM_DRAIN_GRACE_MS)) => {
            tracing::warn!(
                grace_ms = AXUM_DRAIN_GRACE_MS,
                "Connection drain exceeded grace window, proceeding to terminate children"
            );
            Ok(())
        },
    }
}

pub async fn drain(ctx: &AppContext, scheduler: Option<SchedulerHandle>) {
    if let Some(handle) = ctx.event_bridge().get() {
        handle.shutdown().await;
    }

    if let Some(handle) = scheduler
        && let Err(e) = handle.shutdown().await
    {
        tracing::warn!(error = %e, "Scheduler failed to drain cleanly");
    }

    let (background, ()) = tokio::join!(
        ctx.background_tasks()
            .shutdown(Duration::from_millis(BACKGROUND_DRAIN_GRACE_MS)),
        terminate_children(ctx),
    );
    if let DrainOutcome::TimedOut { in_flight } = background {
        tracing::warn!(
            in_flight,
            grace_ms = BACKGROUND_DRAIN_GRACE_MS,
            "Background work still running when the shutdown drain expired"
        );
    }

    if let Err(e) = systemprompt_logging::shutdown_database_logging().await {
        tracing::warn!(error = %e, "Database log writer failed to flush on shutdown");
    }
}

pub async fn terminate_children(ctx: &AppContext) {
    let repo = ctx.service_repository();

    tokio::join!(terminate_agent_children(repo), terminate_mcp_children(repo),);
}

async fn terminate_agent_children(repo: &systemprompt_database::ServiceRepository) {
    let names = match repo.list_all_agent_service_names().await {
        Ok(names) => names,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list agent services for shutdown");
            return;
        },
    };

    futures_util::future::join_all(names.into_iter().map(|name| async move {
        if let Ok(Some(service)) = repo.find_service_by_name(&name).await {
            terminate_service_child(repo, &name, service.pid, ChildKind::Agent).await;
        }
    }))
    .await;
}

async fn terminate_mcp_children(repo: &systemprompt_database::ServiceRepository) {
    let services = match repo.list_mcp_services().await {
        Ok(services) => services,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list MCP services for shutdown");
            return;
        },
    };

    futures_util::future::join_all(services.into_iter().map(|service| async move {
        terminate_service_child(repo, &service.name, service.pid, ChildKind::Mcp).await;
    }))
    .await;
}

async fn terminate_service_child(
    repo: &systemprompt_database::ServiceRepository,
    name: &systemprompt_identifiers::ServiceName,
    pid: Option<i32>,
    kind: ChildKind,
) {
    let Some(pid) = pid.and_then(|p| u32::try_from(p).ok()) else {
        return;
    };
    let grace = Duration::from_millis(CHILD_SHUTDOWN_GRACE_MS);
    match systemprompt_loader::subprocess::stop_owned(pid, kind, name, grace).await {
        Ok(StopOutcome::NotRunning) => {},
        Ok(StopOutcome::NotOurs) => {
            tracing::warn!(
                service = %name,
                pid,
                "Recorded PID is alive but is not our child (recycled/stale); clearing registry row without signalling"
            );
            if let Err(e) = repo.update_service_stopped(name).await {
                tracing::warn!(service = %name, error = %e, "Failed to clear stale service PID");
            }
        },
        Ok(StopOutcome::Stopped(termination)) => {
            tracing::info!(service = %name, pid, ?termination, "Terminated child process group on shutdown");
        },
        Err(e) => {
            tracing::warn!(service = %name, pid, error = %e, "Child process group survived shutdown signal");
        },
    }
}
