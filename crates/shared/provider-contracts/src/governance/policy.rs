//! The governance policy contract and the values it evaluates over.
//!
//! A [`GovernancePolicy`] is contributed by a member as a factory and run by
//! the governance engine over a [`PolicyContext`]; its scanner internals stay
//! in the governance crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::ops::Range;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{CallId, PolicyId, SecretPatternId, SessionId, UserId};

use super::GovernanceParseError;
use super::decision::Decision;
use super::governed::{GovernedInput, GovernedTarget};

/// Where in a governed payload a secret-scanner finding was located.
///
/// [`GovernedInput::location_kind`] supplies `kind` for the governance chain,
/// and `redacted` must already have the credential removed — it is rendered
/// into [`DenyReason`](super::DenyReason) and reaches the audit log.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(feature = "sqlx", sqlx(type_name = "TEXT", rename_all = "lowercase"))]
#[serde(rename_all = "lowercase")]
pub enum AccessScope {
    Admin,
    User,
    Unknown,
}

impl AccessScope {
    #[must_use]
    pub fn from_roles<S: AsRef<str>>(roles: &[S]) -> Self {
        if roles.iter().any(|r| r.as_ref() == "admin") {
            Self::Admin
        } else if roles.iter().any(|r| r.as_ref() == "user") {
            Self::User
        } else {
            Self::Unknown
        }
    }

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
    type Err = GovernanceParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            "unknown" | "" => Ok(Self::Unknown),
            other => Err(GovernanceParseError::UnknownAccessScope(other.to_owned())),
        }
    }
}

#[derive(Debug)]
pub struct PolicyContext<'a> {
    pub target: GovernedTarget,
    pub agent_scope: AgentScope,
    pub access_scope: AccessScope,
    pub roles: &'a [String],
    pub session_id: &'a SessionId,
    pub user_id: &'a UserId,
    pub input: &'a GovernedInput,
    pub call_id: &'a CallId,
    pub rate_counts: &'a RateCounts,
}

impl PolicyContext<'_> {
    #[must_use]
    pub fn with_input<'b>(&'b self, input: &'b GovernedInput) -> PolicyContext<'b> {
        PolicyContext {
            target: self.target.clone(),
            agent_scope: self.agent_scope.clone(),
            access_scope: self.access_scope,
            roles: self.roles,
            session_id: self.session_id,
            user_id: self.user_id,
            input,
            call_id: self.call_id,
            rate_counts: self.rate_counts,
        }
    }
}


/// Each charged window's 1-based position of the current call within it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RateCounts {
    positions: Vec<(PolicyId, u64)>,
}

impl RateCounts {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            positions: Vec::new(),
        }
    }

    #[must_use]
    pub fn charged(positions: impl IntoIterator<Item = (PolicyId, u64)>) -> Self {
        let mut counts = Self::empty();
        for (policy_id, position) in positions {
            counts.record(policy_id, position);
        }
        counts
    }

    pub fn record(&mut self, policy_id: PolicyId, position: u64) {
        self.positions.retain(|(id, _)| *id != policy_id);
        self.positions.push((policy_id, position));
    }

    #[must_use]
    pub fn position(&self, policy_id: &PolicyId) -> Option<u64> {
        self.positions
            .iter()
            .find(|(id, _)| id == policy_id)
            .map(|(_, position)| *position)
    }
}

/// Index into the ordered string surfaces returned by `GovernedInput::strings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretSource {
    pub part_index: usize,
}

/// Credential-free match metadata with UTF-8 byte offsets into its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretFinding {
    pub source: SecretSource,
    pub span: Range<usize>,
    pub pattern_id: SecretPatternId,
}

/// A unit of governance evaluation for one governed call — an MCP tool call or
/// a submitted prompt, per [`PolicyContext::target`].
///
/// Implementations are pure-sync; auditing happens outside the chain.
/// Traced first-deny-wins composition is provided by the governance engine.
///
/// `evaluate` must be **idempotent per [`PolicyContext::call_id`]**: evaluating
/// one call twice yields the same [`Decision`] and leaves the same state behind
/// as evaluating it once. A policy that counts calls therefore counts calls,
/// not evaluations — the two diverge wherever enforcement points nest.
///
/// `prompt_secret_findings` returns `None` unless the policy can locate the
/// credentials behind its own
/// [`DenyReason::SecretLeak`](super::DenyReason::SecretLeak);
/// only a policy that returns `Some` takes part in prompt recovery.
pub trait GovernancePolicy: Send + Sync + fmt::Debug {
    fn id(&self) -> PolicyId;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Decision;
    fn prompt_secret_findings(&self, _input: &GovernedInput) -> Option<Vec<SecretFinding>> {
        None
    }
    fn rate_window(&self) -> Option<RateLimitWindow> {
        None
    }
}
