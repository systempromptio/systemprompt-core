//! API server assembly, lifecycle, and readiness.
//!
//! [`startup`] binds the TCP listener before bootstrap and serves a starting
//! health probe; [`builder`] composes the full axum router and global
//! middleware stack; [`runner`] runs the startup reconciliation, swaps the
//! full router onto the listener, and awaits shutdown. [`ReadinessState`],
//! created by [`bind_and_serve`] and carried by the [`EarlyServer`], is the
//! admission signal the probes and the drain-aware tasks share. Discovery,
//! health, metrics, and route configuration live in the private submodules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod builder;
mod discovery;
pub mod health;
mod health_stats;

mod config_fingerprint;
pub mod health_detail;
pub mod lifecycle;
pub mod metrics;
pub mod ops_metrics;
mod pool_metrics;
mod probes;
pub mod readiness;
mod replica_identity;
pub mod routes;
pub mod runner;
pub mod scheduler_health;
pub mod shutdown;
pub mod startup;

pub use builder::*;
pub use readiness::{ReadinessEvent, ReadinessState};
pub use runner::*;
pub use scheduler_health::SchedulerHealth;
pub use startup::{EarlyServer, bind_and_serve, starting_router};
