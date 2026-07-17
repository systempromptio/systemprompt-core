//! Service-orchestration primitives: process/port lifecycle, state-manager
//! verification, and the reconciler that maps desired vs runtime state to
//! concrete actions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod process_cleanup;
pub mod reconciler;
pub mod state_types;
pub mod state_verifier;
pub mod verified_state;

pub use process_cleanup::{ProcessCleanup, ProcessInfo};
pub use reconciler::{ReconciliationResult, ServiceReconciler};
pub use state_types::{DesiredStatus, RuntimeStatus, ServiceAction, ServiceType};
pub use state_verifier::{DbServiceRecord, ServiceConfig, ServiceStateVerifier};
pub use verified_state::VerifiedServiceState;
