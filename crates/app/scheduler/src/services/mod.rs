//! Service layer for the scheduler crate.
//!
//! Five concerns live here: [`scheduling`] owns the cron scheduler and job
//! dispatch, `job_execution` runs jobs on demand outside the cron loop,
//! [`orchestration`] holds the service-lifecycle reconciler and process/port
//! primitives, `service_management` wraps service-record start/stop
//! bookkeeping, and `plans` computes pure start/restart plans for composition
//! roots. `providers` adapts [`ProcessCleanup`] to the `systemprompt-traits`
//! provider contract so other crates can depend on the trait rather than this
//! crate directly.

mod job_execution;
pub mod orchestration;
mod plans;
mod providers;
pub mod scheduling;
mod service_management;

pub use job_execution::{
    JobBatchReport, JobExecutionService, JobRunReport, JobSelection, parse_job_parameters,
};
pub use orchestration::{
    DbServiceRecord, DesiredStatus, ProcessCleanup, ProcessInfo, ReconciliationResult,
    RuntimeStatus, ServiceAction, ServiceConfig, ServiceReconciler, ServiceStateVerifier,
    ServiceType, VerifiedServiceState,
};
pub use plans::{
    RestartPlan, RestartScope, RestartTarget, ServiceSnapshot, StartupPlan, StartupRequest,
};
pub use scheduling::{SchedulerHandle, SchedulerService};
pub use service_management::{
    OrphanCleanupReport, OrphanDisposition, OrphanOutcome, ServiceManagementService,
};
