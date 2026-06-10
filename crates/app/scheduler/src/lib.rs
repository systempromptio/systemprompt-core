//! `systemprompt-scheduler` — background-job and service-orchestration
//! engine for the systemprompt.io AI governance platform.
//!
//! The crate hosts:
//!
//! - A [`SchedulerService`] that uses [`tokio_cron_scheduler`] to dispatch jobs
//!   registered via [`systemprompt_provider_contracts::submit_job!`].
//! - A small set of built-in jobs ([`BehavioralAnalysisJob`],
//!   [`CleanupInactiveSessionsJob`], …) that drive analytics and security
//!   maintenance.
//! - A [`JobExecutionService`] that runs jobs on demand outside the cron loop
//!   and records each run.
//! - Process- and database-level service reconciliation primitives
//!   ([`ProcessCleanup`], [`ServiceReconciler`], [`ServiceStateVerifier`]),
//!   plus pure start/restart planning ([`StartupPlan`], [`RestartPlan`]) for
//!   composition roots.
//!
//! # Public error surface
//!
//! Every non-trait public API returns
//! [`SchedulerResult<T>`](crate::SchedulerResult) (alias for `Result<T,
//! SchedulerError>`). [`SchedulerError`] composes the
//! `sqlx`, `tokio-cron-scheduler`, `systemprompt-database`,
//! `systemprompt-analytics`, and `systemprompt-users` error types via
//! `#[from]`, plus an `Internal(String)` carve-out for cases where the
//! upstream cause is stringified at the call site rather than typed.
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
    DbServiceRecord, DesiredStatus, JobBatchReport, JobExecutionService, JobRunReport,
    JobSelection, OrphanCleanupReport, OrphanDisposition, OrphanOutcome, ProcessCleanup,
    ProcessInfo, ReconciliationResult, RestartPlan, RestartScope, RestartTarget, RuntimeStatus,
    SchedulerHandle, SchedulerService, ServiceAction, ServiceConfig, ServiceManagementService,
    ServiceReconciler, ServiceSnapshot, ServiceStateVerifier, ServiceType, StartupPlan,
    StartupRequest, VerifiedServiceState, parse_job_parameters,
};
