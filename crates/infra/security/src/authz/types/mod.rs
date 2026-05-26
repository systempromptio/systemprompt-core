//! Wire and storage types for authorization decisions.
//!
//! Types fall into two groups:
//!
//! 1. **Storage** — [`RuleType`], [`Access`], [`AccessRule`] map to columns in
//!    `access_control_rules`. They round-trip through serde and sqlx.
//! 2. **Decision** — [`Decision`] is the in-process resolver output;
//!    [`AuthzRequest`] / [`AuthzDecision`] are the webhook wire format sent to
//!    and parsed back from extension hook handlers.

mod decision;
mod entity_ref;
mod kinds;
mod request;
mod rule;

pub use decision::{Decision, DecisionTag, DenyReason, MatchedBy};
pub use entity_ref::EntityRef;
pub use kinds::{Access, EntityKind, RuleType};
pub use request::{AuthzContext, AuthzDecision, AuthzRequest};
pub use rule::{AccessRule, EntityRow};
