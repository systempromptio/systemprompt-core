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
//!   ([`ServiceReconciler`], [`ServiceStateVerifier`], and the marker-verified
//!   stops in [`services::orchestration::supervision`]), plus pure
//!   start/restart planning ([`StartupPlan`], [`RestartPlan`]) for composition
//!   roots.
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
//! always compiled. It has no platform-specific code: process and port
//! supervision is [`systemprompt_loader::subprocess`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod error;
pub mod extension;
pub mod jobs;
pub mod models;
pub mod repository;
pub mod services;

pub use error::{SchedulerError, SchedulerResult};
pub use extension::SchedulerExtension;

pub use jobs::otlp_export::{
    ExportReport as OtlpExportReport, SignalReport as OtlpSignalReport,
    export_now as otlp_export_now,
};
pub use jobs::{
    BehavioralAnalysisJob, CleanupEmptyContextsJob, CleanupInactiveSessionsJob, DatabaseCleanupJob,
    GhostSessionCleanupJob, MaliciousIpBlacklistJob, NoJsCleanupJob, OtlpExportJob,
};
pub use models::{JobConfig, JobRunRecord, JobStatus, ScheduledJob, SchedulerConfig, SkippedJob};
pub use repository::otlp::{OtlpExportState, OtlpExportStateRepository};
pub use repository::{JobRepository, SchedulerRepository};
pub use services::{
    ApiListenerStop, DbServiceRecord, DesiredStatus, JobBatchReport, JobExecutionService,
    JobRunReport, JobSelection, OrphanCleanupReport, OrphanDisposition, OrphanOutcome,
    ReconciliationResult, RestartPlan, RestartScope, RestartTarget, RuntimeStatus, SchedulerHandle,
    SchedulerService, SchedulerStartup, ServiceAction, ServiceConfig, ServiceManagementService,
    ServiceReconciler, ServiceSnapshot, ServiceStateVerifier, ServiceType, StartupPlan,
    StartupRequest, VerifiedServiceState, child_kind, parse_job_parameters, port_holders,
    stop_api_listeners, stop_owned_port_holders, unknown_job_names, wait_for_port_free,
};
