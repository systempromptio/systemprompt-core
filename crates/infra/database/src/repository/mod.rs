//! Repositories owned by the database crate itself.
//!
//! Only repositories that are part of the platform-level schema (services
//! registry, instance leases, release stamps) live here. Domain repositories
//! live in their respective domain crates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod advisory_lock;
pub mod base;
pub mod coordination;
pub mod lease;
pub mod ownership;
pub mod release_stamp;
pub mod service;

pub use advisory_lock::{EXTENSION_LOCK_CLASS, SessionAdvisoryLock, SessionAdvisoryLocks};
pub use base::PgDbPool;
pub use coordination::CoordinationRepositories;
pub use lease::InstanceLeaseRepository;
pub use release_stamp::{
    ReleaseGate, ReleaseRun, ReleaseStampError, ReleaseStampRepository, StampOutcome, release_key,
};
pub use service::{
    CLAIM_DEAD_PEER_SECS, CreateServiceInput, INSTANCE_CLAIM_CLASS, INSTANCE_DEAD_AFTER_SECS,
    InstanceClaim, InstanceClaimError, ServiceConfig, ServiceModule, ServiceRepository,
    ServiceStatus, UpsertServiceProcessInput,
};
