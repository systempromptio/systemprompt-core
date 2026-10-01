//! Repository over the platform-wide `services` registry table.
//!
//! Split into:
//! - `model` — row type ([`ServiceConfig`]) and write-input
//!   ([`CreateServiceInput`], [`UpsertServiceProcessInput`]).
//! - `repo` — [`ServiceRepository`] construction, lookup and writes.
//! - `listing` — read-side listings by status and module.
//! - `maintenance` — heartbeat, stale-row cleanup and dead-instance reaping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod listing;
mod maintenance;
mod model;
mod repo;

pub use model::{CreateServiceInput, ServiceConfig, UpsertServiceProcessInput};
pub use repo::ServiceRepository;
pub use systemprompt_models::services::{ServiceModule, ServiceStatus};
