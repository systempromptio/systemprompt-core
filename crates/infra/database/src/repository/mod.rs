//! Repositories owned by the database crate itself.
//!
//! Only repositories that are part of the platform-level schema (services
//! registry, cleanup utilities, database introspection) live here. Domain
//! repositories live in their respective domain crates.

pub mod base;
pub mod cleanup;
pub mod info;
pub mod service;

pub use base::PgDbPool;
pub use cleanup::CleanupRepository;
pub use info::DatabaseInfoRepository;
pub use service::{CreateServiceInput, ServiceConfig, ServiceRepository};
