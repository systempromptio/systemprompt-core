//! API server assembly, lifecycle, and readiness.
//!
//! [`startup`] binds the TCP listener before bootstrap and serves a starting
//! health probe; [`builder`] composes the full axum router and global
//! middleware stack; [`runner`] runs the startup reconciliation, swaps the
//! full router onto the listener, and awaits shutdown. The readiness
//! signalling surface ([`is_ready`], [`signal_ready`], [`wait_for_ready`]) is
//! used by external health probes. Discovery, health, metrics, and route
//! configuration live in the private submodules.

pub mod builder;
mod discovery;
mod health;

#[cfg(feature = "test-api")]
pub mod test_api {
    #[cfg(target_os = "linux")]
    pub use super::health::parse_proc_status_kb;
    pub use super::health::{audit_log_stats, database_stats, human_bytes, table_stats};
    pub use super::health_detail::handle_health_detail;
}
mod health_detail;
mod lifecycle;
pub mod metrics;
pub mod readiness;
mod routes;
pub mod runner;
pub mod scheduler_health;
mod shutdown;
pub mod startup;

#[cfg(feature = "test-api")]
pub use lifecycle::reconciliation_test_api;
#[cfg(feature = "test-api")]
pub use shutdown::test_api as shutdown_test_api;

pub use builder::*;
pub use readiness::{
    ReadinessEvent, get_readiness_receiver, init_readiness, is_ready, signal_ready,
    signal_shutdown, wait_for_ready,
};
pub use runner::*;
pub use startup::{EarlyServer, bind_and_serve, starting_router};
