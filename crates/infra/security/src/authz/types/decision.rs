//! Authorization decision types and structured deny reasons.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::fmt;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{McpToolName, PolicyId, SecretPatternId, UserId};
use thiserror::Error;

use super::entity_ref::EntityRef;
use super::kinds::RuleType;
use crate::policy::types::{AccessScope, RateLimitWindow, SecretLocation};

/// Why an [`super::request::AuthzRequest`] was allowed. Carries enough
/// structure for the audit row to attribute the decision without re-deriving
/// it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MatchedBy {
    UserAllow,
    RoleAllow {
        role: String,
    },
    AttributeAllow {
        rule_type: RuleType,
        value: String,
    },
    DefaultIncluded,
    PolicyAllow {
        policy_id: PolicyId,
        detail: Cow<'static, str>,
    },
}

/// Structured deny rationale.
///
/// Variants cover both the user→entity resolver
/// (`UserDeny`, `RoleDeny`, `NotAssigned`, `UnknownEntity`),
/// the hook plane (`HookUnavailable`), and the tool-use governance chain
/// (`SecretLeak`, `ScopeViolation`, `ToolBlocked`, `RateLimitExceeded`). The
/// human-readable `#[error]` strings double as the `reason` column in the
/// `governance_decisions` audit row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DenyReason {
    #[error("user {user_id} explicitly denied for {entity}")]
    UserDeny {
        entity: EntityRef,
        user_id: UserId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        justification: Option<String>,
    },
    #[error("role {role} denied for {entity}")]
    RoleDeny {
        entity: EntityRef,
        role: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        justification: Option<String>,
    },
    #[error("{rule_type} {value} denied for {entity}")]
    AttributeDeny {
        entity: EntityRef,
        rule_type: RuleType,
        value: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        justification: Option<String>,
    },
    #[error(
        "{entity}: not assigned to user {user_id} with roles {roles:?} (no allow rule; \
         default_included = false). Add an allow rule in services/access-control/roles.yaml."
    )]
    NotAssigned {
        entity: EntityRef,
        user_id: UserId,
        roles: Vec<String>,
    },
    #[error(
        "{entity}: unknown to access control. Add an entity row via the publish pipeline or \
         roles.yaml."
    )]
    UnknownEntity { entity: EntityRef },
    // Why: `detail` carries the underlying failure so the audit row can tell a
    // transient database fault from a malformed rule. Without it every fault on
    // this plane writes a byte-identical row and the cause survives only in a
    // log line. `serde(default)` so rows written before the field parse back.
    #[error("authz hook unavailable for policy {policy}: {detail}")]
    HookUnavailable {
        policy: String,
        #[serde(default)]
        detail: String,
    },
    #[error("{detail}")]
    PolicyViolation {
        policy: String,
        detail: Cow<'static, str>,
    },
    #[error("secret detected: {pattern_name} at {location}")]
    SecretLeak {
        pattern_id: SecretPatternId,
        pattern_name: Cow<'static, str>,
        location: SecretLocation,
    },
    #[error("tool {tool} requires {required} scope")]
    ScopeViolation {
        tool: McpToolName,
        required: AccessScope,
    },
    #[error("tool {tool} blocked by list {list_id}")]
    ToolBlocked { tool: McpToolName, list_id: String },
    #[error("rate limit {window:?} exceeded; retry after {retry_after_ms}ms")]
    RateLimitExceeded {
        window: RateLimitWindow,
        retry_after_ms: u64,
    },
}

/// Why a governed call was held for a human decision instead of being
/// allowed or denied outright.
///
/// A `Pending` verdict is *not* a refusal: the chain has found nothing wrong
/// with the call, only that policy requires a named human to authorise it
/// before it runs. The enforcement point is responsible for parking the call
/// and resuming it — see the `require_approval` policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PendingReason {
    #[error("tool {tool} requires human approval (matched {rule})")]
    ApprovalRequired { tool: McpToolName, rule: String },
}

/// The verdict of one policy chain run.
///
/// `Warn` is the observability verdict: a policy in `mode: warn` found what it
/// would normally refuse, the finding is recorded verbatim, and the call
/// proceeds anyway. It carries the same [`DenyReason`] the enforcing form
/// would have carried, so a warn row and a deny row are directly comparable —
/// that is the whole point of warn mode, which exists so tunables can be
/// adjusted from real traffic instead of guesses.
///
/// Every enforcement point must treat `Warn` as an allow. A site that lets it
/// fall into a deny arm turns warn mode back into enforcement silently, which
/// is the one failure this type exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "lowercase")]
pub enum Decision {
    Allow { matched_by: MatchedBy },
    Warn { reason: DenyReason },
    Deny { reason: DenyReason },
    Pending { reason: PendingReason },
}

impl Decision {
    #[must_use]
    pub const fn tag(&self) -> DecisionTag {
        match self {
            Self::Allow { .. } => DecisionTag::Allow,
            Self::Warn { .. } => DecisionTag::Warn,
            Self::Deny { .. } => DecisionTag::Deny,
            Self::Pending { .. } => DecisionTag::Pending,
        }
    }

    // Why: the predicate every enforcement point should use. Matching on
    // `Allow` alone turns warn mode back into enforcement silently, which is
    // the one failure the `Warn` variant exists to prevent.
    #[must_use]
    pub const fn permits(&self) -> bool {
        matches!(self, Self::Allow { .. } | Self::Warn { .. })
    }
}

/// Discriminant-only view of [`Decision`] / [`super::request::AuthzDecision`],
/// bound to the `governance_decisions.decision` column.
///
/// Typing the column at the Rust boundary couples it to the SQL CHECK
/// allow-list; adding a `Decision` variant without extending the constraint
/// fails the build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DecisionTag {
    Allow,
    Warn,
    Deny,
    Pending,
}

impl DecisionTag {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Warn => "warn",
            Self::Deny => "deny",
            Self::Pending => "pending",
        }
    }
}

impl fmt::Display for DecisionTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&super::request::AuthzDecision> for DecisionTag {
    fn from(d: &super::request::AuthzDecision) -> Self {
        match d {
            super::request::AuthzDecision::Allow => Self::Allow,
            super::request::AuthzDecision::Deny { .. } => Self::Deny,
        }
    }
}
