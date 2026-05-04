//! `systemprompt-scheduler` — background-job and service-orchestration
//! engine for the systemprompt.io AI governance platform.
//!
//! The crate hosts:
//!
//! - A [`SchedulerService`] that uses
//!   [`tokio_cron_scheduler`] to dispatch jobs registered via
//!   [`systemprompt_provider_contracts::submit_job!`].
//! - A small set of built-in jobs ([`BehavioralAnalysisJob`],
//!   [`CleanupInactiveSessionsJob`], …) that drive analytics and security
//!   maintenance.
//! - Process- and database-level service reconciliation primitives
//!   ([`ProcessCleanup`], [`ServiceReconciler`], [`ServiceStateManager`]).
//!
//! # Public error surface
//!
//! Every non-trait public API returns
//! [`SchedulerResult<T>`](crate::SchedulerResult) (alias for `Result<T,
//! SchedulerError>`). [`SchedulerError`] composes the
//! `sqlx`, `tokio-cron-scheduler`, `systemprompt-database`,
//! `systemprompt-analytics`, and `systemprompt-users` error types via
//! `#[from]`, plus an `Other(#[from] anyhow::Error)` catch-all for upstream
//! callers that still propagate `anyhow`.
//!
//! Provider-trait bodies (`Job::execute`, …) keep returning
//! [`systemprompt_provider_contracts::ProviderResult`] for ABI parity with
//! the trait contract; a `From<SchedulerError> for ProviderError` impl makes
//! `?` propagation transparent inside job bodies.
//!
//! # Feature flags
//!
//! This crate has no Cargo feature gates of its own — all functionality is
//! always compiled. Conditional compilation is limited to platform-specific
//! `#[cfg(unix)]` / `#[cfg(windows)]` shims inside
//! [`services::orchestration::ProcessCleanup`].
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod error;
pub mod extension;
pub mod jobs;
pub mod models;
pub mod repository;
pub mod services;

pub use error::{SchedulerError, SchedulerResult};
pub use extension::SchedulerExtension;

pub use jobs::{
    BehavioralAnalysisJob, CleanupEmptyContextsJob, CleanupInactiveSessionsJob, DatabaseCleanupJob,
    GhostSessionCleanupJob, MaliciousIpBlacklistJob, NoJsCleanupJob,
};
pub use models::{JobConfig, JobStatus, ScheduledJob, SchedulerConfig};
pub use repository::{JobRepository, SchedulerRepository};
pub use services::{
    DbServiceRecord, DesiredStatus, ProcessCleanup, ProcessInfo, ReconciliationResult,
    RuntimeStatus, SchedulerService, ServiceAction, ServiceConfig, ServiceManagementService,
    ServiceReconciler, ServiceStateManager, ServiceType, VerifiedServiceState,
};
