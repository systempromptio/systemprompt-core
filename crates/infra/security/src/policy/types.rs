//! Shared types for the unified governance plane.
//!
//! These types support the governance chain ([`super::GovernancePolicy`]) and
//! feed into the typed deny variants in [`crate::authz::types::DenyReason`].
//! They live here (and not in `authz/types.rs`) because they describe the
//! *governed-call* enforcement plane — secret scans, scope checks, blocklists,
//! rate limits — which is orthogonal to the user→entity allow/deny resolver.
//! What a governed call targets and carries lives in [`super::governed`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{CallId, PolicyId, SessionId, UserId};

use super::governed::{GovernedInput, GovernedTarget};
use crate::authz::error::AuthzError;
use crate::authz::types::Decision;

/// Where in a governed payload a secret-scanner finding was located.
///
/// [`GovernedInput::location_kind`] supplies `kind` for the governance chain,
/// and `redacted` must already have the credential removed — it is rendered
/// into [`crate::authz::types::DenyReason`] and reaches the audit log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretLocation {
    pub kind: String,
    pub path: String,
    pub redacted: String,
}

impl SecretLocation {
    pub fn new(
        kind: impl Into<String>,
        path: impl Into<String>,
        redacted: impl Into<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            path: path.into(),
            redacted: redacted.into(),
        }
    }
}

impl fmt::Display for SecretLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "{} ({})", self.kind, self.redacted)
        } else {
            write!(f, "{}.{} ({})", self.kind, self.path, self.redacted)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitWindow {
    pub name: String,
    pub seconds: u64,
    pub limit: u64,
}

/// Scope of an agent invocation for governance evaluation. Agents may run
/// either inside an authenticated user session or under a system/service
/// identity (cron, replay, internal scheduler).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentScope {
    User { user_id: UserId },
    System,
}

impl AgentScope {
    #[must_use]
    pub const fn user_id(&self) -> Option<&UserId> {
        match self {
            Self::User { user_id } => Some(user_id),
            Self::System => None,
        }
    }
}

/// Permission tier carried alongside [`AgentScope`] in [`PolicyContext`].
///
/// `AgentScope` answers "who is acting" (user vs system process identity);
/// `AccessScope` answers "what permission tier is granted to this invocation"
/// (admin, plain user, unknown). The two are orthogonal — a system actor may
/// have any tier, a user actor may be admin or plain — so they live as
/// separate fields rather than a cartesian enum. `Unknown` is the fallback when
/// an agent card declares no `oauth.scopes` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AccessScope {
    Admin,
    User,
    Unknown,
}

impl AccessScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::User => "user",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for AccessScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AccessScope {
    type Err = AuthzError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            "unknown" | "" => Ok(Self::Unknown),
            other => Err(AuthzError::Validation(format!(
                "unknown access scope: {other}"
            ))),
        }
    }
}

#[derive(Debug)]
pub struct PolicyContext<'a> {
    pub target: GovernedTarget,
    pub agent_scope: AgentScope,
    pub access_scope: AccessScope,
    pub session_id: &'a SessionId,
    pub user_id: &'a UserId,
    pub input: &'a GovernedInput,
    /// Identity of the logical call, stable across every evaluation of it.
    ///
    /// One call is legitimately evaluated more than once — an enforcement point
    /// behind another still has to run the chain, because it is reachable by
    /// callers that never passed the first. A policy that accumulates state
    /// uses this to tell "the same call again" from "another call".
    pub call_id: &'a CallId,
}

/// A unit of governance evaluation for one governed call — an MCP tool call or
/// a submitted prompt, per [`PolicyContext::target`].
///
/// Implementations are pure-sync; auditing happens outside the chain.
/// Traced first-deny-wins composition is provided by
/// [`super::GovernanceEngine`].
///
/// `evaluate` must be **idempotent per [`PolicyContext::call_id`]**: evaluating
/// one call twice yields the same [`Decision`] and leaves the same state behind
/// as evaluating it once. A policy that counts calls therefore counts calls,
/// not evaluations — the two diverge wherever enforcement points nest.
pub trait GovernancePolicy: Send + Sync + fmt::Debug {
    fn id(&self) -> PolicyId;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Decision;
}
