//! `systemprompt-scheduler` — background-job and service-orchestration
//! engine for the systemprompt.io AI governance platform.
//!
//! The crate hosts:
//!
//! - A [`SchedulerService`] that uses [`tokio_cron_scheduler`] to dispatch jobs
//!   of the admitted composition
//!   ([`AdmittedSchedule`](systemprompt_extension::AdmittedSchedule)).
//! - The kernel's built-in jobs ([`DatabaseCleanupJob`], …), contributed by
//!   [`SchedulerExtension`]. Capability members contribute their own jobs, and
//!   retention purges and projection drains to the kernel jobs that iterate
//!   them.
//! - A [`JobExecutionService`] that runs jobs on demand outside the cron loop
//!   and records each run.
//! - A [`JobPruneService`] that retires `scheduled_jobs` rows no admitted job
//!   names, on an operator's request. A node's boot never deletes a row; it
//!   schedules the admitted jobs and ignores the rest.
//! - Process- and database-level service reconciliation primitives
//!   ([`ServiceReconciler`], [`ServiceStateVerifier`], and the marker-verified
//!   stops in [`services::orchestration::supervision`]), plus pure
//!   start/restart planning ([`StartupPlan`], [`RestartPlan`]) for composition
//!   roots.
//!
//! # Delivery guarantee
//!
//! A cron tick is keyed by the scheduled instant it belongs to, and a run is
//! claimed by inserting its `scheduled_job_runs` row in the statement that
//! judges the claim. With `scheduler.distributed_lock` on, a cluster-scoped
//! job runs exactly once per instant across every replica while the database
//! is reachable, and a node-scoped job exactly once per instant on each
//! replica; under a database outage a tick that cannot claim is skipped, so a
//! run is at most once and never twice. Instants no replica ran are handled at
//! start by the job's [`MissedRunPolicy`](systemprompt_traits::MissedRunPolicy)
//! (default `Skip`). With the lock off (one process) every tick runs and
//! nothing is claimed. A bootstrap run is per boot, and a manual run
//! (`infra jobs run`) runs now, under the job's advisory lock only.
//!
//! # Public error surface
//!
//! Every non-trait public API returns
//! [`SchedulerResult<T>`](crate::SchedulerResult) (alias for `Result<T,
//! SchedulerError>`). [`SchedulerError`] composes the
//! `sqlx`, `tokio-cron-scheduler`, `systemprompt-database` and
//! `systemprompt-users` error types via `#[from]`.
//!
//! Provider-trait bodies (`Job::execute`, …) keep returning
//! [`systemprompt_provider_contracts::ProviderResult`] for ABI parity with
//! the trait contract; a `From<SchedulerError> for ProviderError` impl makes
//! `?` propagation transparent inside job bodies.
//!
//! # Feature flags
//!
//! - `otel` — the `otlp_export` job, which ships the audit trail to an OTLP
//!   collector. Without it the job is not registered.
//!
//! The crate has no platform-specific code: process and port supervision is
//! [`systemprompt_loader::subprocess`].
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

pub use jobs::DatabaseCleanupJob;
#[cfg(feature = "otel")]
pub use jobs::OtlpExportJob;
#[cfg(feature = "otel")]
pub use jobs::otlp_export::{
    ExportReport as OtlpExportReport, SignalReport as OtlpSignalReport,
    export_now as otlp_export_now,
};
pub use models::{JobConfig, JobRunRecord, JobStatus, ScheduledJob, SchedulerConfig, SkippedJob};
pub use repository::otlp::{OtlpExportState, OtlpExportStateRepository};
pub use repository::{
    JobRepository, JobRunRepository, LicenceCheckinRepository, LicenceCheckinState,
    LockKeyCollision, RunClaim, RunKey, SchedulerRepository, find_lock_key_collision,
};
pub use services::{
    ApiListenerStop, CronInstants, DbServiceRecord, DesiredStatus, DynSchedulerClock,
    JobBatchReport, JobExecutionService, JobPruneService, JobRunReport, JobSelection,
    OrphanCleanupReport, OrphanDisposition, OrphanOutcome, ReconciliationResult, RestartPlan,
    RestartScope, RestartTarget, SchedulerClock, SchedulerHandle, SchedulerService,
    SchedulerStartup, ServiceAction, ServiceConfig, ServiceManagementService, ServiceReconciler,
    ServiceSnapshot, ServiceStateVerifier, ServiceType, StartupPlan, StartupRequest, SystemClock,
    VerifiedService, VerifiedServiceState, child_kind, parse_job_parameters, port_holders,
    stop_api_listeners, stop_owned_port_holders, unknown_job_names, unlisted_job_names,
    wait_for_port_free,
};
