//! The rendezvous behind [`Decision::Pending`][crate::authz::Decision].
//!
//! The `require_approval` policy renders the verdict; this module is where a
//! held call actually waits. Two processes meet on one `approval_requests`
//! row: the MCP server that parked the call blocks on it, and the admin
//! console resolves it.
//!
//! The row is keyed by [`CallId`][systemprompt_identifiers::CallId] because
//! [`GovernancePolicy::evaluate`][crate::policy::GovernancePolicy::evaluate] is
//! contractually idempotent per call. An MRTR retry re-enters governance with
//! the same call id, so it must rejoin the approval it already opened rather
//! than opening a second one — otherwise every retry round would reset the
//! wait and no approval could ever be granted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod repository;
mod waiter;

pub use repository::{
    ApprovalRepository, ApprovalRequest, ApprovalStatus, ApprovalVerdict, NewApprovalRequest,
    args_digest,
};
pub use waiter::{ApprovalOutcome, wait_for_decision};
