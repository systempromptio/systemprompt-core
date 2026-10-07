//! Repository over the platform-wide `services` registry table.
//!
//! Split into:
//! - `model` — row type ([`ServiceConfig`]) and write-input
//!   ([`CreateServiceInput`], [`UpsertServiceProcessInput`]).
//! - `repo` — [`ServiceRepository`] construction, lookup and writes.
//! - `listing` — read-side listings by status and module.
//! - `claim` — exclusive boot-time claim on this replica's instance id.
//! - `maintenance` — heartbeat, stale-row cleanup and dead-instance reaping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod claim;
mod listing;
mod maintenance;
mod model;
mod repo;

pub use claim::{INSTANCE_CLAIM_CLASS, InstanceClaim, InstanceClaimError};
pub use model::{CreateServiceInput, ServiceConfig, UpsertServiceProcessInput};
pub use repo::ServiceRepository;
pub use systemprompt_manifest::services::{ServiceModule, ServiceStatus};
