//! The per-tick lease loop behind [`super::process_ownership`]: it reads the
//! reachable workers and the processes the configuration wants, settles each
//! process's lease, and hands starts and stops to the process actuator.
//!
//! On shutdown it stops every process it holds before releasing that
//! process's lease, so a peer that claims the lease never runs the process
//! beside this one. The stops run in parallel within the teardown's
//! background grace, alongside the drain's own child termination; a process
//! not confirmed stopped by then, or with no pid recorded yet because its
//! start is still in flight, keeps its lease, which expires after
//! [`LEASE_TTL`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_database::{InstanceLeaseRepository, ServiceModule};
use systemprompt_identifiers::{InstanceId, ServiceName};
use systemprompt_loader::subprocess::{self, ChildKind, StopOutcome};
use systemprompt_manifest::profile::NodeRole;
use systemprompt_runtime::AppContext;
use systemprompt_traits::LeaseRepository;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::process_actuator::{ProcessCommand, spawn_process_actuator};
use super::process_ownership::{
    FAILED_START_BACKOFF, KeyAction, LEASE_TTL, OwnershipStep, ProcessKey, RENEW_EVERY, key_action,
    ownership_step, renewal_overdue, with_self,
};

struct LeaseLoop {
    ctx: AppContext,
    me: InstanceId,
    leases: Arc<InstanceLeaseRepository>,
    actuator: UnboundedSender<ProcessCommand>,
    held: HashMap<ProcessKey, Instant>,
    backoff: HashMap<ProcessKey, Instant>,
    settled: Vec<InstanceId>,
}

pub(in crate::services::server) fn start_leased_supervision(ctx: &AppContext) {
    let (failed_tx, mut failed_rx) = unbounded_channel();
    let actuator = spawn_process_actuator(ctx, failed_tx);
    let tasks = ctx.background_tasks().clone();
    let me = ctx.config().instance_id.clone();
    let mut state = LeaseLoop {
        ctx: ctx.clone(),
        leases: Arc::clone(&ctx.coordination_repositories().leases),
        settled: vec![me.clone()],
        me,
        actuator,
        held: HashMap::new(),
        backoff: HashMap::new(),
    };
    tasks.spawn_cancellable("process_leases", move |cancel| async move {
        let mut tick = tokio::time::interval(RENEW_EVERY);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                () = cancel.cancelled() => break,
                _ = tick.tick() => {},
            }
            state.absorb_failures(&mut failed_rx);
            state.tick().await;
        }
        state.stop_and_release_all().await;
    });
}

impl LeaseLoop {
    fn absorb_failures(&mut self, failed: &mut UnboundedReceiver<ProcessKey>) {
        while let Ok(key) = failed.try_recv() {
            self.held.remove(&key);
            self.backoff
                .insert(key, Instant::now() + FAILED_START_BACKOFF);
        }
        let now = Instant::now();
        self.backoff.retain(|_, until| *until > now);
    }

    async fn tick(&mut self) {
        let workers = match self
            .ctx
            .service_repository()
            .list_reachable_instances(NodeRole::Worker, LEASE_TTL.as_secs() as i64)
            .await
        {
            Ok(workers) => with_self(workers, &self.me),
            Err(error) => {
                tracing::warn!(error = %error, "reachable workers could not be read");
                self.settled.clone()
            },
        };
        let wanted = match desired_processes(&self.ctx) {
            Ok(wanted) => wanted,
            Err(error) => {
                tracing::warn!(error = %error, "supervised processes could not be listed");
                return;
            },
        };
        let unwanted: Vec<ProcessKey> = self
            .held
            .keys()
            .filter(|key| !wanted.contains(key))
            .cloned()
            .collect();
        for key in unwanted {
            self.held.remove(&key);
            self.command(ProcessCommand::Stop { key, release: true });
        }
        for key in wanted {
            self.settle(key, &workers).await;
        }
        self.settled = workers;
    }

    async fn settle(&mut self, key: ProcessKey, workers: &[InstanceId]) {
        let held = self.held.contains_key(&key);
        if !held && self.backoff.contains_key(&key) {
            return;
        }
        let lease = key.lease_key();
        match key_action(&lease, &self.me, workers, &self.settled, held) {
            KeyAction::Skip => {},
            KeyAction::HandOver => {
                tracing::info!(service = %key.name(), "handing process over to its preferred worker");
                self.held.remove(&key);
                self.command(ProcessCommand::Stop { key, release: true });
            },
            KeyAction::Claim | KeyAction::Renew => match self.leases.claim(&lease, LEASE_TTL).await
            {
                Err(error) => {
                    tracing::warn!(service = %key.name(), error = %error, "process lease claim failed");
                    if self
                        .held
                        .get(&key)
                        .is_some_and(|renewed| renewal_overdue(renewed.elapsed()))
                    {
                        self.held.remove(&key);
                        self.command(ProcessCommand::Stop {
                            key,
                            release: false,
                        });
                    }
                },
                Ok(claim) => match ownership_step(held, claim.as_ref(), &self.me) {
                    OwnershipStep::Acquired => {
                        tracing::info!(service = %key.name(), "process lease acquired");
                        self.held.insert(key.clone(), Instant::now());
                        self.command(ProcessCommand::Start(key));
                    },
                    OwnershipStep::Renewed => {
                        self.held.insert(key, Instant::now());
                    },
                    OwnershipStep::Standby => {},
                    OwnershipStep::Lost => {
                        tracing::warn!(service = %key.name(), "process lease lost to a peer");
                        self.held.remove(&key);
                        self.command(ProcessCommand::Stop {
                            key,
                            release: false,
                        });
                    },
                },
            },
        }
    }

    fn command(&self, command: ProcessCommand) {
        if self.actuator.send(command).is_err() {
            tracing::warn!("process actuator has stopped");
        }
    }

    async fn stop_and_release_all(&mut self) {
        let timings = self.ctx.config().shutdown;
        let budget = timings
            .background_grace()
            .saturating_sub(RELEASE_MARGIN)
            .min(timings.child_grace() + RELEASE_MARGIN);
        let held: Vec<ProcessKey> = self.held.drain().map(|(key, _)| key).collect();
        let ctx = &self.ctx;
        let leases = &self.leases;
        futures_util::future::join_all(held.into_iter().map(|key| async move {
            match tokio::time::timeout(budget, stop_held(ctx, &key, timings.child_grace())).await {
                Ok(true) => {
                    if let Err(error) = leases.release(&key.lease_key()).await {
                        tracing::warn!(service = %key.name(), error = %error, "process lease release failed");
                    }
                },
                Ok(false) | Err(_) => tracing::warn!(
                    service = %key.name(),
                    "process not confirmed stopped on shutdown; leaving its lease to expire"
                ),
            }
        }))
        .await;
    }
}

const RELEASE_MARGIN: Duration = Duration::from_secs(1);

async fn stop_held(ctx: &AppContext, key: &ProcessKey, grace: Duration) -> bool {
    let service = match ctx
        .service_repository()
        .find_service_by_name(key.name())
        .await
    {
        Ok(service) => service,
        Err(error) => {
            tracing::warn!(service = %key.name(), error = %error, "process row unreadable on shutdown");
            return false;
        },
    };
    let Some(pid) = service
        .and_then(|row| row.pid)
        .and_then(|pid| u32::try_from(pid).ok())
    else {
        tracing::warn!(service = %key.name(), "no pid recorded on shutdown; leaving its lease to expire");
        return false;
    };
    let kind = match key.module() {
        ServiceModule::Mcp => ChildKind::Mcp,
        ServiceModule::Agent => ChildKind::Agent,
    };
    match subprocess::stop_owned(pid, kind, key.name(), grace).await {
        Ok(StopOutcome::NotRunning | StopOutcome::Stopped(_) | StopOutcome::NotOurs) => true,
        Err(error) => {
            tracing::warn!(service = %key.name(), pid, error = %error, "process survived shutdown stop");
            false
        },
    }
}

fn desired_processes(ctx: &AppContext) -> anyhow::Result<Vec<ProcessKey>> {
    let mut wanted: Vec<ProcessKey> = ctx
        .mcp_registry()
        .get_managed_servers()?
        .iter()
        .map(|server| ProcessKey::new(ServiceModule::Mcp, server.service_name()))
        .collect();
    wanted.extend(
        AgentRegistry::new(&ctx.services_state().view())
            .list_enabled_agents(ctx.config().is_cloud)
            .into_iter()
            .map(|agent| ProcessKey::new(ServiceModule::Agent, ServiceName::new(agent.name))),
    );
    Ok(wanted)
}
