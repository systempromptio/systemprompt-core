//! Service layer for the scheduler crate.
//!
//! [`config_apply`] is the explicit three-way-merge apply of declared config
//! planes and [`config_reconcile`] the report-only drift pass. Five more
//! concerns live here: [`scheduling`] owns the cron scheduler and job
//! dispatch, `job_execution` runs jobs on demand outside the cron loop,
//! [`orchestration`] holds the service-lifecycle reconciler and process/port
//! primitives, `service_management` wraps service-record start/stop
//! bookkeeping, and `plans` computes pure start/restart plans for composition
//! roots.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod config_apply;
pub mod config_reconcile;
mod job_execution;
pub mod job_metrics;
pub mod orchestration;
mod plans;
pub mod scheduling;
mod service_management;

pub use config_apply::{
    APPLY_MODE, ApplyOptions, ConfigApplyService, EntityMerge, FILE_WINS_MODE, MergeReport,
    PlaneMerge, PlanePatch, Verdict, file_wins_verdict, merge_plane, verdict,
};
pub use config_reconcile::{
    AppliedPlane, ConfigReconcileService, PlaneDiff, PlaneOutcome, PlaneReport,
    RECONCILE_LOCK_WAIT, ReconcileReport, ReconcileTrigger,
};
pub use job_execution::{
    JobBatchReport, JobExecutionService, JobRunReport, JobSelection, parse_job_parameters,
};
pub use orchestration::{
    ApiListenerStop, DbServiceRecord, DesiredStatus, ReconciliationResult, ServiceAction,
    ServiceConfig, ServiceReconciler, ServiceStateVerifier, ServiceType, VerifiedService,
    VerifiedServiceState, child_kind, port_holders, stop_api_listeners, stop_owned_port_holders,
    wait_for_port_free,
};
pub use plans::{
    RestartPlan, RestartScope, RestartTarget, ServiceSnapshot, StartupPlan, StartupRequest,
};
pub use scheduling::{
    SchedulerHandle, SchedulerService, SchedulerStartup, unknown_job_names, unlisted_job_names,
};
pub use service_management::{
    OrphanCleanupReport, OrphanDisposition, OrphanOutcome, ServiceManagementService,
};
