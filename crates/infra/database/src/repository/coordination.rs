//! The cross-replica coordination repositories, built once per process at
//! the composition root and handed to extensions through their context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::InstanceId;

use super::advisory_lock::SessionAdvisoryLocks;
use super::lease::InstanceLeaseRepository;
use crate::DbPool;

#[derive(Debug, Clone)]
pub struct CoordinationRepositories {
    pub leases: Arc<InstanceLeaseRepository>,
    pub locks: Arc<SessionAdvisoryLocks>,
}

impl CoordinationRepositories {
    pub fn new(db: &DbPool, instance_id: InstanceId) -> Self {
        Self {
            leases: Arc::new(InstanceLeaseRepository::new(db, instance_id)),
            locks: Arc::new(SessionAdvisoryLocks::new(db)),
        }
    }
}
