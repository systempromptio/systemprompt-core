//! API server assembly, lifecycle, and readiness.
//!
//! Builds the axum router and global middleware stack ([`builder`]), runs the
//! startup reconciliation and serving loop ([`runner`]), and exposes the
//! readiness signalling surface ([`is_ready`], [`signal_ready`],
//! [`wait_for_ready`]) used by external health probes. Discovery, health,
//! metrics, and route configuration live in the private submodules.

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
