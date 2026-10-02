//! Service-orchestration primitives: process/port lifecycle, state-manager
//! verification, and the reconciler that maps desired vs runtime state to
//! concrete actions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod reconciler;
pub mod service_records;
pub mod state_types;
pub mod state_verifier;
pub mod supervision;
pub mod verified_state;

pub use reconciler::{ReconciliationResult, ServiceReconciler};
pub use service_records::{DbServiceRecord, ServiceConfig};
pub use state_types::{DesiredStatus, RuntimeStatus, ServiceAction, ServiceType};
pub use state_verifier::ServiceStateVerifier;
pub use supervision::{
    child_kind, port_holders, stop_owned_port_holders, stop_port_listeners, wait_for_port_free,
};
pub use verified_state::VerifiedServiceState;
