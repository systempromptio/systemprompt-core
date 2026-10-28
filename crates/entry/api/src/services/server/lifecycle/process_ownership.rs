//! One `worker` replica at a time owns and supervises the MCP and agent
//! processes.
//!
//! Every worker claims the [`PROCESS_SUPERVISOR_LEASE`] each
//! [`RENEW_EVERY`]. The replica that wins it reconciles the MCP servers and
//! agents and keeps renewing; the others stand by and keep claiming, so a
//! standby takes over as soon as the owner's instance claim lapses (a crash)
//! or, at the latest, one [`LEASE_TTL`] after its last renewal. The TTL
//! outlasts a slow first reconcile, during which the owner does not renew. An
//! owner that loses the lease, or cannot renew it before it would expire,
//! requests a graceful restart: its shutdown stops the processes it spawned,
//! and a replica that no longer owns them must not keep them running beside
//! the new owner's. `api` replicas find the owner's processes through the
//! service registry and forward to its advertised address.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use systemprompt_identifiers::{InstanceId, LeaseKey};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{Lease, LeaseRepository};

use crate::services::server::runner::supervise_processes;

pub const PROCESS_SUPERVISOR_LEASE: &str = "process-supervisor";
pub const LEASE_TTL: Duration = Duration::from_secs(180);
pub const RENEW_EVERY: Duration = Duration::from_secs(15);

/// What one claim of the supervisor lease means for this replica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipStep {
    Acquired,
    Renewed,
    Standby,
    Lost,
}

pub fn ownership_step(owned: bool, claim: Option<&Lease>, me: &InstanceId) -> OwnershipStep {
    let holds = claim.is_some_and(|lease| &lease.owner == me);
    match (owned, holds) {
        (false, true) => OwnershipStep::Acquired,
        (true, true) => OwnershipStep::Renewed,
        (false, false) => OwnershipStep::Standby,
        (true, false) => OwnershipStep::Lost,
    }
}

pub const fn renewal_overdue(since_last_renewal: Duration) -> bool {
    since_last_renewal.as_secs() + RENEW_EVERY.as_secs() >= LEASE_TTL.as_secs()
}

pub(in crate::services::server) fn start_leased_supervision(ctx: &AppContext) {
    let tasks = ctx.background_tasks().clone();
    let ctx = ctx.clone();
    tasks.spawn_cancellable("process_supervisor_lease", move |cancel| async move {
        let key = LeaseKey::new(PROCESS_SUPERVISOR_LEASE);
        let leases = std::sync::Arc::clone(&ctx.coordination_repositories().leases);
        let me = ctx.config().instance_id.clone();
        let mut owned = false;
        let mut last_renewal = Instant::now();
        let mut tick = tokio::time::interval(RENEW_EVERY);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                () = cancel.cancelled() => break,
                _ = tick.tick() => {},
            }
            let claim = match leases.claim(&key, LEASE_TTL).await {
                Ok(claim) => claim,
                Err(error) => {
                    tracing::warn!(error = %error, "process supervisor lease claim failed");
                    if owned && renewal_overdue(last_renewal.elapsed()) {
                        ctx.request_restart(
                            "process supervisor lease could not be renewed before expiry",
                        );
                        break;
                    }
                    continue;
                },
            };
            match ownership_step(owned, claim.as_ref(), &me) {
                OwnershipStep::Acquired => {
                    owned = true;
                    last_renewal = Instant::now();
                    tracing::info!(instance_id = %me, "process supervisor lease acquired");
                    if let Err(error) = supervise_processes(&ctx).await {
                        tracing::error!(error = %error, "process supervision failed to start");
                        ctx.request_restart("process supervision failed to start");
                        break;
                    }
                },
                OwnershipStep::Renewed => last_renewal = Instant::now(),
                OwnershipStep::Standby => {},
                OwnershipStep::Lost => {
                    tracing::error!(instance_id = %me, "process supervisor lease lost to a peer");
                    ctx.request_restart("process supervisor lease lost to a peer");
                    break;
                },
            }
        }
        if owned && let Err(error) = leases.release(&key).await {
            tracing::warn!(error = %error, "process supervisor lease release failed");
        }
    });
}
