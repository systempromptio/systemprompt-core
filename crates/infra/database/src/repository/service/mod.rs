//! Repository over the platform-wide `services` registry table.
//!
//! Split into:
//! - `model` — row type ([`ServiceConfig`]) and write-input
//!   ([`CreateServiceInput`]).
//! - `repo` — [`ServiceRepository`] async methods.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod model;
mod repo;

pub use model::{CreateServiceInput, ServiceConfig};
pub use repo::ServiceRepository;
