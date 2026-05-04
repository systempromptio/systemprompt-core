//! Repository over the platform-wide `services` registry table.
//!
//! Split into:
//! - `model` — row type ([`ServiceConfig`]) and write-input
//!   ([`CreateServiceInput`]).
//! - `repo` — [`ServiceRepository`] async methods.

mod model;
mod repo;

pub use model::{CreateServiceInput, ServiceConfig};
pub use repo::ServiceRepository;
