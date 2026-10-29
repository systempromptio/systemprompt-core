//! `worker` replicas share the MCP and agent processes between them, one
//! lease per process.
//!
//! Each process has its own `<module>:<name>` lease ([`ProcessKey`]).
//! Rendezvous hashing over the reachable workers
//! ([`super::rendezvous::preferred_owner`]) names the worker each process
//! belongs on, so every worker reaches the same answer without coordinating.
//! Every [`RENEW_EVERY`] a worker renews the leases it holds, claims the ones
//! it is preferred for, and hands over a held process once its preferred
//! owner has been reachable for a whole tick: it stops the process, then
//! releases the lease, and the new owner claims it on its next tick. A claim
//! also wins a lease that expired or whose holder is no longer alive, which is
//! how a crashed worker's processes move once it leaves the reachable set. A
//! worker that loses a lease, or cannot renew it before it would expire,
//! stops that process alone. `api` replicas find each process's holder
//! through the service registry and forward to its advertised address.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_database::ServiceModule;
use systemprompt_identifiers::{InstanceId, LeaseKey, ServiceName};
use systemprompt_traits::Lease;

use super::rendezvous::preferred_owner;

pub const LEASE_TTL: Duration = Duration::from_secs(45);
pub const RENEW_EVERY: Duration = Duration::from_secs(15);
pub const FAILED_START_BACKOFF: Duration = Duration::from_secs(60);

/// One supervised process, named by its registry module and service name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProcessKey {
    module: ServiceModule,
    name: ServiceName,
}

impl ProcessKey {
    pub const fn new(module: ServiceModule, name: ServiceName) -> Self {
        Self { module, name }
    }

    pub const fn module(&self) -> ServiceModule {
        self.module
    }

    pub const fn name(&self) -> &ServiceName {
        &self.name
    }

    pub fn lease_key(&self) -> LeaseKey {
        LeaseKey::new(format!("{}:{}", self.module.as_str(), self.name.as_str()))
    }
}

/// What one supervision tick does with one process's lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Claim,
    Renew,
    HandOver,
    Skip,
}

pub fn key_action(
    key: &LeaseKey,
    me: &InstanceId,
    workers: &[InstanceId],
    settled: &[InstanceId],
    held: bool,
) -> KeyAction {
    let preferred = preferred_owner(key.as_str(), workers);
    let mine = preferred.is_none_or(|owner| owner == me);
    match (held, mine) {
        (true, false) if preferred.is_some_and(|owner| settled.contains(owner)) => {
            KeyAction::HandOver
        },
        (true, _) => KeyAction::Renew,
        (false, true) => KeyAction::Claim,
        (false, false) => KeyAction::Skip,
    }
}

/// What one claim of a process lease means for this replica.
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

pub fn with_self(mut workers: Vec<InstanceId>, me: &InstanceId) -> Vec<InstanceId> {
    if !workers.contains(me) {
        workers.push(me.clone());
    }
    workers.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    workers.dedup();
    workers
}
