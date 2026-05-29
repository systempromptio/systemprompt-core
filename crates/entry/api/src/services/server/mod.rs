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
mod health_detail;
mod lifecycle;
pub mod metrics;
pub mod readiness;
mod routes;
pub mod runner;
mod shutdown;

pub use builder::*;
pub use readiness::{
    ReadinessEvent, get_readiness_receiver, init_readiness, is_ready, signal_ready,
    signal_shutdown, wait_for_ready,
};
pub use runner::*;
