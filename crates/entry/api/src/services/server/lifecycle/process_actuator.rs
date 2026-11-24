//! Starts and stops the processes this worker's leases cover, one command at
//! a time and off the lease loop, so a slow start never delays a renewal.
//!
//! Commands run in the order the lease loop sends them, so a handover's stop
//! always finishes before that process could be started here again. A
//! handover releases the lease only after the process has stopped, so the new
//! owner never runs it beside this one, and every release is announced so the
//! worker that should take the process claims it at once. A process that fails
//! to start has its lease released and is reported back, and the lease loop
//! holds off reclaiming it for
//! [`super::process_ownership::FAILED_START_BACKOFF`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::{CoordinationRepositories, ServiceModule, ServiceRepository};
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{
    BackgroundTasks, DynServiceProcessSet, Lease, LeaseRepository, RestartMode,
    ServiceProcessError, ServiceProcessKind, StopMode,
};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use super::agents::process_set;
use super::process_ownership::ProcessKey;

#[derive(Debug)]
pub(super) enum ProcessCommand {
    Start {
        key: ProcessKey,
        lease: Lease,
    },
    Stop {
        key: ProcessKey,
        release: Option<Lease>,
    },
}

struct ProcessSets {
    mcp: Option<DynServiceProcessSet>,
    agents: Option<DynServiceProcessSet>,
}

impl ProcessSets {
    fn of(ctx: &AppContext) -> Self {
        let sets = Self {
            mcp: process_set(ctx, ServiceProcessKind::McpServer),
            agents: process_set(ctx, ServiceProcessKind::Agent),
        };
        if sets.mcp.is_none() {
            tracing::warn!("no MCP process set is admitted; leased MCP servers are not actuated");
        }
        if sets.agents.is_none() {
            tracing::warn!("no agent process set is admitted; leased agents are not actuated");
        }
        sets
    }

    fn of_module(
        &self,
        module: ServiceModule,
    ) -> Result<&DynServiceProcessSet, ServiceProcessError> {
        let (set, kind) = match module {
            ServiceModule::Mcp => (&self.mcp, ServiceProcessKind::McpServer),
            ServiceModule::Agent => (&self.agents, ServiceProcessKind::Agent),
        };
        set.as_ref()
            .ok_or(ServiceProcessError::Unavailable { kind })
    }
}

pub(super) fn spawn_process_actuator(
    ctx: &AppContext,
    failed: UnboundedSender<ProcessKey>,
) -> Result<UnboundedSender<ProcessCommand>, MissingDependency> {
    let (commands, mut inbox) = unbounded_channel();
    let state = ctx.kernel().state();
    let services = Arc::clone(state.get::<Arc<ServiceRepository>>()?);
    let leases = Arc::clone(&state.get::<Arc<CoordinationRepositories>>()?.leases);
    let sets = ProcessSets::of(ctx);
    state.get::<BackgroundTasks>()?.clone().spawn_cancellable(
        "process_actuator",
        move |cancel| async move {
            loop {
                let command = tokio::select! {
                    () = cancel.cancelled() => break,
                    command = inbox.recv() => match command {
                        Some(command) => command,
                        None => break,
                    },
                };
                apply(&services, &sets, leases.as_ref(), &failed, command).await;
            }
        },
    );
    Ok(commands)
}

async fn apply(
    services: &ServiceRepository,
    sets: &ProcessSets,
    leases: &dyn LeaseRepository,
    failed: &UnboundedSender<ProcessKey>,
    command: ProcessCommand,
) {
    {
        match command {
            ProcessCommand::Start { key, lease } => {
                if let Err(error) = start(sets, &key).await {
                    tracing::error!(
                        service = %key.name(),
                        module = %key.module(),
                        error = %error,
                        "owned process failed to start; releasing its lease"
                    );
                    release_and_announce(services, leases, &key, &lease).await;
                    if failed.send(key).is_err() {
                        tracing::debug!("process lease loop has stopped");
                    }
                }
            },
            ProcessCommand::Stop { key, release } => {
                if let Err(error) = stop(sets, &key).await {
                    tracing::error!(
                        service = %key.name(),
                        module = %key.module(),
                        error = %error,
                        "owned process failed to stop; leaving its lease to expire"
                    );
                    return;
                }
                tracing::info!(service = %key.name(), module = %key.module(), "owned process stopped");
                if let Some(lease) = release {
                    release_and_announce(services, leases, &key, &lease).await;
                }
            },
        }
    }
}

pub(super) async fn release_and_announce(
    services: &ServiceRepository,
    leases: &dyn LeaseRepository,
    key: &ProcessKey,
    lease: &Lease,
) {
    if let Err(error) = leases.release(lease).await {
        tracing::warn!(service = %key.name(), error = %error, "process lease release failed");
        return;
    }
    if let Err(error) = services.announce_process_released(&lease.key).await {
        tracing::warn!(
            service = %key.name(),
            error = %error,
            "process lease release not announced; peers claim it at their next retry"
        );
    }
}

async fn start(sets: &ProcessSets, key: &ProcessKey) -> Result<(), ServiceProcessError> {
    let set = sets.of_module(key.module())?;
    let runs = match key.module() {
        ServiceModule::Mcp => set.start(Some(key.name())).await?,
        ServiceModule::Agent => set.restart(Some(key.name()), RestartMode::Restart).await?,
    };
    for run in runs {
        run.outcome?;
    }
    tracing::info!(service = %key.name(), module = %key.module(), "owned process started");
    Ok(())
}

async fn stop(sets: &ProcessSets, key: &ProcessKey) -> Result<(), ServiceProcessError> {
    let Ok(set) = sets.of_module(key.module()) else {
        return Ok(());
    };
    let mode = match key.module() {
        ServiceModule::Mcp => StopMode::Disable,
        ServiceModule::Agent => StopMode::Remove,
    };
    set.stop(Some(key.name()), mode).await
}
