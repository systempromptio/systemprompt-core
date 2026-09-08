//! API server assembly, lifecycle, and readiness.
//!
//! [`startup`] binds the TCP listener before bootstrap and serves a starting
//! health probe; [`builder`] composes the full axum router and global
//! middleware stack; [`runner`] runs the startup reconciliation, swaps the
//! full router onto the listener, and awaits shutdown. The readiness
//! signalling surface ([`is_ready`], [`signal_ready`], [`wait_for_ready`]) is
//! used by external health probes. Discovery, health, metrics, and route
//! configuration live in the private submodules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod builder;
mod discovery;
pub mod health;

pub mod health_detail;
pub mod lifecycle;
pub mod metrics;
mod probes;
pub mod readiness;
mod routes;
pub mod runner;
pub mod scheduler_health;
pub mod shutdown;
pub mod startup;

pub use builder::*;
pub use readiness::{
    ReadinessEvent, get_readiness_receiver, init_readiness, is_ready, signal_ready,
    signal_shutdown, wait_for_ready,
};
pub use runner::*;
pub use startup::{EarlyServer, bind_and_serve, starting_router};
