//! Service-orchestration primitives: process/port lifecycle, state-manager
//! verification, and the reconciler that maps desired vs runtime state to
//! concrete actions.

pub mod process_cleanup;
pub mod reconciler;
pub mod state_manager;
pub mod state_types;
pub mod verified_state;

pub use process_cleanup::{ProcessCleanup, ProcessInfo};
pub use reconciler::{ReconciliationResult, ServiceReconciler};
pub use state_manager::{DbServiceRecord, ServiceConfig, ServiceStateManager};
pub use state_types::{DesiredStatus, RuntimeStatus, ServiceAction, ServiceType};
pub use verified_state::VerifiedServiceState;
