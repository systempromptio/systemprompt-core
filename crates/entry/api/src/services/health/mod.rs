//! Service health monitoring for managed subprocesses.
//!
//! [`HealthChecker`] performs on-demand checks; [`ProcessMonitor`] runs a
//! background loop that reconciles tracked PIDs against the running process
//! table and marks crashed services, aggregating results into a
//! [`HealthSummary`] of per-module [`ModuleHealth`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod checker;
pub mod monitor;

pub use checker::HealthChecker;
pub use monitor::{HealthSummary, ModuleHealth, ProcessMonitor};
