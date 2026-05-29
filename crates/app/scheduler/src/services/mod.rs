//! Service layer for the scheduler crate.
//!
//! Three concerns live here: [`scheduling`] owns the cron scheduler and job
//! dispatch, [`orchestration`] holds the service-lifecycle reconciler and
//! process/port primitives, and `service_management` wraps service-record
//! start/stop bookkeeping. `providers` adapts [`ProcessCleanup`] to the
//! `systemprompt-traits` provider contract so other crates can depend on the
//! trait rather than this crate directly.

pub mod orchestration;
mod providers;
pub mod scheduling;
mod service_management;

pub use orchestration::{
    DbServiceRecord, DesiredStatus, ProcessCleanup, ProcessInfo, ReconciliationResult,
    RuntimeStatus, ServiceAction, ServiceConfig, ServiceReconciler, ServiceStateVerifier,
    ServiceType, VerifiedServiceState,
};
pub use scheduling::{SchedulerHandle, SchedulerService};
pub use service_management::ServiceManagementService;
