//! The per-tick lease loop behind [`super::process_ownership`]: it reads the
//! reachable workers and the processes the configuration wants, settles each
//! process's lease, and hands starts and stops to the process actuator.
//!
//! On shutdown it first withdraws this worker's presence, so peers stop
//! preferring it at their next tick, then stops every process it holds before
//! releasing that process's lease, so a peer that claims the lease never runs
//! the process beside this one: until the release, a peer's claim of a key it
//! now prefers finds the lease held and stands by. The withdrawal is repeated
//! after the releases because the heartbeat, cancelled at the same moment, may
//! have a presence write in flight. A worker that stands by on a key it is
//! preferred for retries the claim every
//! [`CLAIM_RETRY_EVERY`](super::process_ownership::CLAIM_RETRY_EVERY) instead
//! of waiting a whole [`RENEW_EVERY`]; retries only claim, they never renew or
//! hand over, so the hand-over still waits for a peer settled over a whole
//! tick. The stops run in parallel within the teardown's
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
    FAILED_START_BACKOFF, KeyAction, LEASE_TTL, OwnershipStep, ProcessKey, RENEW_EVERY, TickKind,
    key_action, next_tick, ownership_step, renewal_overdue, with_self,
};

struct LeaseLoop {
    ctx: AppContext,
    me: InstanceId,
    leases: Arc<InstanceLeaseRepository>,
    actuator: UnboundedSender<ProcessCommand>,
    held: HashMap<ProcessKey, Instant>,
    backoff: HashMap<ProcessKey, Instant>,
    settled: Vec<InstanceId>,
    pending: bool,
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
        pending: false,
    };
    tasks.spawn_cancellable("process_leases", move |cancel| async move {
        let mut last_full: Option<Instant> = None;
        let mut wait = Duration::ZERO;
        loop {
            tokio::select! {
                () = cancel.cancelled() => break,
                () = tokio::time::sleep(wait) => {},
            }
            state.absorb_failures(&mut failed_rx);
            let kind = if last_full.is_none_or(|at| at.elapsed() >= RENEW_EVERY) {
                last_full = Some(Instant::now());
                TickKind::Full
            } else {
                TickKind::ClaimOnly
            };
            state.tick(kind).await;
            let since_full = last_full.map_or(Duration::ZERO, |at| at.elapsed());
            wait = next_tick(since_full, state.pending);
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

    async fn tick(&mut self, kind: TickKind) {
        self.pending = false;
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
        if kind == TickKind::Full {
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
        }
        for key in wanted {
            self.settle(key, &workers, kind).await;
        }
        if kind == TickKind::Full {
            self.settled = workers;
        }
    }

    async fn settle(&mut self, key: ProcessKey, workers: &[InstanceId], kind: TickKind) {
        let held = self.held.contains_key(&key);
        if !held && self.backoff.contains_key(&key) {
            return;
        }
        let lease = key.lease_key();
        let action = key_action(&lease, &self.me, workers, &self.settled, held);
        if kind == TickKind::ClaimOnly && action != KeyAction::Claim {
            return;
        }
        match action {
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
                    OwnershipStep::Standby => self.pending = true,
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

    async fn withdraw_presence(&self) {
        if let Err(error) = self.ctx.service_repository().delete_presence().await {
            tracing::warn!(error = %error, "worker presence could not be withdrawn on shutdown; peers wait for it to go stale");
        }
    }

    async fn stop_and_release_all(&mut self) {
        self.withdraw_presence().await;
        let timings = self.ctx.config().shutdown;
        let budget = timings
            .background_grace()
            .saturating_sub(RELEASE_MARGIN)
            .min(timings.child_grace() + RELEASE_MARGIN);
        let held = std::mem::take(&mut self.held).into_keys();
        let ctx = &self.ctx;
        let leases = &self.leases;
        futures_util::future::join_all(held.map(|key| async move {
            let stopped =
                tokio::time::timeout(budget, stop_held(ctx, &key, timings.child_grace())).await;
            if stopped == Ok(true) {
                if let Err(error) = leases.release(&key.lease_key()).await {
                    tracing::warn!(service = %key.name(), error = %error, "process lease release failed");
                }
            } else {
                tracing::warn!(
                    service = %key.name(),
                    "process not confirmed stopped on shutdown; leaving its lease to expire"
                );
            }
        }))
        .await;
        self.withdraw_presence().await;
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
