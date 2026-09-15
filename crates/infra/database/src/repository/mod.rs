//! Repositories owned by the database crate itself.
//!
//! Only repositories that are part of the platform-level schema (services
//! registry) live here. Domain repositories live in their
//! respective domain crates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod base;
pub mod service;

pub use base::PgDbPool;
pub use service::{CreateServiceInput, ServiceConfig, ServiceRepository};
