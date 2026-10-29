//! Startup phases that report their progress on the startup event channel.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use systemprompt_runtime::AppContext;
use systemprompt_scheduler::services::SchedulerHandle;
use systemprompt_traits::{Phase, StartupEvent, StartupEventExt, StartupEventSender};

use super::lifecycle::{initialize_scheduler, reconcile_agents};
use super::scheduler_health::SchedulerHealth;

pub(super) async fn run_agents_phase(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<()> {
    if let Some(tx) = events {
        tx.phase_started(Phase::Agents);
    }
    match reconcile_agents(ctx, events).await {
        Ok(started_count) => {
            if let Some(tx) = events {
                send_startup_event(
                    tx,
                    StartupEvent::AgentReconciliationComplete {
                        running: started_count,
                        total: started_count,
                    },
                );
                tx.phase_completed(Phase::Agents);
            }
            Ok(())
        },
        Err(e) => Err(fail_phase(
            events,
            Phase::Agents,
            format!("Agent reconciliation failed: {e}"),
            e,
        )),
    }
}

pub(super) async fn run_scheduler_phase(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
    scheduler_health: &SchedulerHealth,
) -> Result<Option<SchedulerHandle>> {
    if let Some(tx) = events {
        tx.phase_started(Phase::Scheduler);
    }
    match initialize_scheduler(ctx, events, scheduler_health).await {
        Ok(handle) => {
            if let Some(tx) = events {
                tx.phase_completed(Phase::Scheduler);
            }
            Ok(handle)
        },
        Err(e) => Err(fail_phase(
            events,
            Phase::Scheduler,
            format!("Scheduler initialization failed: {e}"),
            e,
        )),
    }
}

pub(super) fn fail_phase(
    events: Option<&StartupEventSender>,
    phase: Phase,
    message: String,
    error: anyhow::Error,
) -> anyhow::Error {
    if let Some(tx) = events {
        tx.phase_failed(phase, error.to_string());
        send_startup_event(
            tx,
            StartupEvent::Error {
                message,
                fatal: true,
            },
        );
    }
    error
}

pub(super) fn send_startup_event(tx: &StartupEventSender, event: StartupEvent) {
    if tx.unbounded_send(event).is_err() {
        tracing::debug!("Startup event receiver dropped");
    }
}
