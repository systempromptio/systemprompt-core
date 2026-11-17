//! The governance policy contract shared between the governance engine and
//! the members that contribute policies.
//!
//! [`GovernancePolicy`] evaluates one governed call ([`PolicyContext`]) to a
//! [`Decision`]. The values it reads and returns — the governed input and
//! target, the decision and deny reasons with the entity kinds they name, the
//! agent and access scopes, the rate counts and windows, and the
//! credential-free secret findings — are plain data and live here so a member
//! can implement a policy without depending on the governance crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod decision;
mod entity_ref;
mod governed;
mod kinds;
mod policy;

pub use decision::{Decision, DecisionTag, DenyReason, MatchedBy, PendingReason};
pub use entity_ref::EntityRef;
pub use governed::{
    GovernedInput, GovernedScalar, GovernedString, GovernedTarget, McpToolInput,
    PROMPT_TARGET_NAME, PromptPart, UNKNOWN_TARGET_NAME,
};
pub use kinds::{Access, EntityKind, RuleType, RuleTypeError};
pub use policy::{
    AccessScope, AgentScope, GovernancePolicy, PolicyContext, RateCounts, RateLimitWindow,
    SecretFinding, SecretLocation, SecretSource,
};

/// A governance value parsed from its stored text that names no known
/// variant.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GovernanceParseError {
    #[error("invalid access value: {0}")]
    InvalidAccess(String),
    #[error("unknown entity_type: {0}")]
    UnknownEntityKind(String),
    #[error("unknown access scope: {0}")]
    UnknownAccessScope(String),
}
