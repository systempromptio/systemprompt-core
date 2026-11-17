//! Wire and storage types for authorization decisions.
//!
//! Types fall into two groups:
//!
//! 1. **Storage** — [`RuleType`], [`Access`], [`AccessRule`] map to columns in
//!    `access_control_rules`. They round-trip through serde and sqlx.
//! 2. **Decision** — [`Decision`] is the in-process resolver output;
//!    [`AuthzRequest`] / [`AuthzDecision`] are the webhook wire format sent to
//!    and parsed back from extension hook handlers.
//!
//! [`EntityRef::from_kind_and_id`] is the one polymorphic constructor for
//! entity references; [`SubjectRef`] is its counterpart on the subject side.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod request;
mod request_document;
mod rule;
mod subject_ref;

pub use request::{AuthzContext, AuthzDecision, AuthzRequest};
pub use rule::{AccessRule, EntityRow};
pub use subject_ref::SubjectRef;
pub use systemprompt_provider_contracts::governance::{
    Access, Decision, DecisionTag, DenyReason, EntityKind, EntityRef, MatchedBy, PendingReason,
    RuleType, RuleTypeError,
};
